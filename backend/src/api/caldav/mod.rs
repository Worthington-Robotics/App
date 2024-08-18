use std::{collections::HashMap, fmt::Display, path::PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use rocket::{http::Status, response::content::RawXml};

use crate::events::Event;

/// A single calendar
pub struct Calendar<'e> {
	events: HashMap<String, &'e Event>,
}

impl<'e> Calendar<'e> {
	pub fn new(events: impl Iterator<Item = &'e Event>) -> Self {
		Self {
			events: events.map(|x| (x.id.clone(), x)).collect(),
		}
	}

	pub fn serve(
		&self,
		request: PathBuf,
		body: &str,
		cal_id: &str,
	) -> Result<RawXml<String>, Status> {
		let _ = body;

		let components: Vec<_> = request.components().collect();

		if components.is_empty() {
			return Ok(RawXml(format!(
				r#"
<multistatus xmlns="DAV:">
  <response xmlns="DAV:">
    <href>/cal/{cal_id}/</href>
    <propstat>
      <prop>
        <current-user-principal xmlns="DAV:">
          <href xmlns="DAV:">/cal/{cal_id}/principal/</href>
        </current-user-principal>
      </prop>
      <status>HTTP/1.1 200 OK</status>
    </propstat>
  </response>
</multistatus>"#
			)));
		}

		if components.len() == 1 && components[0].as_os_str() == "principal" {
			return Ok(RawXml(format!(
				r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<multistatus xmlns="DAV:">
  <response xmlns="DAV:">
    <href>/cal/{cal_id}/principal/</href>
      <propstat>
         <prop>
            <calendar-home-set xmlns="urn:ietf:params:xml:ns:caldav">
              <href xmlns="DAV:">/cal/{cal_id}/calendars/</href>
            </calendar-home-set>
        </prop>
        <status>HTTP/1.1 200 OK</status>
      </propstat>
      <propstat>
          <prop>
            <group-membership xmlns="DAV:"/>
          </prop>
          <status>HTTP/1.1 404 Not Found</status>
      </propstat>
  </response>
</multistatus>"#
			)));
		}

		if components.len() == 1 && components[0].as_os_str() == "calendars" {
			return Ok(RawXml(format!(
				r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<multistatus xmlns="DAV:">
  <response xmlns="DAV:">
    <href>/cal/{cal_id}/calendars/</href>
    <propstat>
      <prop>
        <current-user-privilege-set xmlns="DAV:">
          <privilege>
              <read/>
          </privilege>
        </current-user-privilege-set>
        <resourcetype xmlns="DAV:">
          <collection/>
        </resourcetype>
        <displayname xmlns="DAV:">User name</displayname>
        <supported-calendar-component-set xmlns="urn:ietf:params:xml:ns:caldav">
          <comp name='VEVENT' xmlns='urn:ietf:params:xml:ns:caldav'/>
          <comp name='VTODO' xmlns='urn:ietf:params:xml:ns:caldav'/>
          <comp name='VFREEBUSY' xmlns='urn:ietf:params:xml:ns:caldav'/>
        </supported-calendar-component-set>
      </prop>
      <status>HTTP/1.1 200 OK</status>
    </propstat>
	</response>
	<response xmlns="DAV:">
    <href>/cal/{cal_id}/calendar/</href>
    <propstat>
      <prop>
        <current-user-privilege-set xmlns="DAV:">
          <privilege>
              <read/>
          </privilege>
        </current-user-privilege-set>
        <resourcetype xmlns="DAV:">
          <collection/>
          <calendar xmlns="urn:ietf:params:xml:ns:caldav"/>
        </resourcetype>
        <displayname xmlns="DAV:">New calendars</displayname>
        <calendar-color xmlns="http://apple.com/ns/ical/">#FF2D55FF</calendar-color>
        <supported-calendar-component-set xmlns="urn:ietf:params:xml:ns:caldav">
          <comp name='VEVENT' xmlns='urn:ietf:params:xml:ns:caldav'/>
        </supported-calendar-component-set>
      </prop>
      <status>HTTP/1.1 200 OK</status>
    </propstat>
  </response>
</multistatus>"#
			)));
		}

		if components.len() == 1 && components[0].as_os_str() == "calendar" {
			//
			let out = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<multistatus xmlns="DAV:">
	{{responses}}
</multistatus>"#;

			let mut events_string = String::new();
			for event in self.events.values() {
				let ical_data = format_ical(event);
				let response = format!(
					r#"<response>
		<href>/cal/{cal_id}/event/{}</href>
		<propstat>
			<prop>
				<getetag xmlns="DAV:">"lrgcxs0m"</getetag>
				<calendar-data xmlns="urn:ietf:params:xml:ns:caldav">{ical_data}</calendar-data>
			</prop>
			<status>HTTP/1.1 200 OK</status>
		</propstat>
	</response>"#,
					event.id
				);

				events_string.push_str(&response);
			}
			let out = out.replace("{{responses}}", &events_string);

			return Ok(RawXml(out.into()));
		}

		Ok(RawXml(String::new()))
	}
}

fn format_ical(event: &Event) -> String {
	let start = if let Ok(start_date) = DateTime::parse_from_rfc2822(&event.date) {
		format!("DTSTART:{}", ical_date(start_date))
	} else {
		String::new()
	};
	let end = if let Some(end_date) = event
		.end_date
		.as_ref()
		.and_then(|x| DateTime::parse_from_rfc2822(&x).ok())
	{
		format!("DTEND:{}", ical_date(end_date))
	} else {
		String::new()
	};

	format!(
		r#"
	<![CDATA[BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//worbots4145.org/app//WorBots Calendar//EN
CALSCALE:GREGORIAN
METHOD:PUBLISH
BEGIN:VEVENT
SUMMARY:{0}
UID:{1}
SEQUENCE:0
STATUS:CONFIRMED
TRANSP:TRANSPARENT
{2}
{3}
DTSTAMP:20150421T141403
URL:https://worbots4145.org/app/event/{1}
END:VEVENT
END:VCALENDAR
]]>
	"#,
		event.name, event.id, start, end
	)
}

pub fn ical_date<T: TimeZone>(date: DateTime<T>) -> String
where
	T::Offset: Display,
{
	date.with_timezone(&Utc)
		.format("%Y%m%dT%H%M%SZ")
		.to_string()
}
