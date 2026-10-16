//! What the organism is told: one line of text per message, the same lines lampbus forwards to a controller.

/// A change of power.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    On,
    Off,
    Toggle,
}

/// What an agent is doing; each state is a colour the organism is pulled toward (see `organism.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Working,
    Done,
    NeedsYou,
    Error,
    Idle,
}

/// One message.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// A person chose a colour, with a brightness if they gave one.
    Look {
        rgb: [f64; 3],
        percent: Option<f64>,
    },
    /// A person changed only the brightness.
    Brightness(f64),
    /// A person turned the lamp on or off.
    Power(Power),
    /// A person asked for something that is not a plain light (an effect, an animation): the control object, as JSON.
    Act(String),
    /// What an agent is doing.
    Agent(Agent),
    /// Put the organism to sleep or wake it.
    Sleep,
    Wake,
    /// Report on the organism.
    Status,
}

fn number(s: Option<&str>, what: &str) -> Result<f64, String> {
    s.ok_or_else(|| format!("missing {what}"))?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{what} is not a number"))
}

/// Parse one line.
pub fn parse(line: &str) -> Result<Msg, String> {
    let line = line.trim();
    let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    match verb {
        "person" => parse_person(rest),
        "agent" => match rest {
            "working" => Ok(Msg::Agent(Agent::Working)),
            "done" => Ok(Msg::Agent(Agent::Done)),
            "needs-you" => Ok(Msg::Agent(Agent::NeedsYou)),
            "error" => Ok(Msg::Agent(Agent::Error)),
            "idle" => Ok(Msg::Agent(Agent::Idle)),
            other => Err(format!("unknown agent state {other:?}")),
        },
        "sleep" => Ok(Msg::Sleep),
        "wake" => Ok(Msg::Wake),
        "status" => Ok(Msg::Status),
        "" => Err("empty message".into()),
        other => Err(format!("unknown message {other:?}")),
    }
}

fn parse_person(rest: &str) -> Result<Msg, String> {
    let (what, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let args = args.trim();
    let mut it = args.split_whitespace();
    match what {
        "look" => {
            let rgb = [
                number(it.next(), "red")?,
                number(it.next(), "green")?,
                number(it.next(), "blue")?,
            ];
            let percent = it
                .next()
                .map(|p| number(Some(p), "brightness"))
                .transpose()?;
            Ok(Msg::Look {
                rgb: rgb.map(|c| c.clamp(0.0, 255.0)),
                percent: percent.map(|p| p.clamp(0.0, 100.0)),
            })
        }
        "brightness" => Ok(Msg::Brightness(
            number(it.next(), "brightness")?.clamp(0.0, 100.0),
        )),
        "power" => match it.next() {
            Some("on") => Ok(Msg::Power(Power::On)),
            Some("off") => Ok(Msg::Power(Power::Off)),
            Some("toggle") => Ok(Msg::Power(Power::Toggle)),
            _ => Err("power takes on, off or toggle".into()),
        },
        "act" if args.starts_with('{') => Ok(Msg::Act(args.to_string())),
        "act" => Err("act takes a JSON object".into()),
        other => Err(format!("unknown person message {other:?}")),
    }
}

/// A screen frame as lampbus's simple UDP input takes it: `r g b`, `r g b percent`, or `r g b hi lo` (hundredths of a percent).
pub fn screen_frame(d: &[u8]) -> Option<([f64; 3], f64)> {
    let c = |v: &u8| f64::from(*v);
    match d {
        [r, g, b] => Some(([c(r), c(g), c(b)], 100.0)),
        [r, g, b, p] => Some(([c(r), c(g), c(b)], f64::from((*p).min(100)))),
        [r, g, b, hi, lo] => Some((
            [c(r), c(g), c(b)],
            f64::from(u16::from_be_bytes([*hi, *lo]).min(10_000)) / 100.0,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_chooses_a_light() {
        assert_eq!(
            parse("person look 255 136 0 40.0"),
            Ok(Msg::Look {
                rgb: [255.0, 136.0, 0.0],
                percent: Some(40.0)
            })
        );
        assert_eq!(
            parse("person look 1 2 3"),
            Ok(Msg::Look {
                rgb: [1.0, 2.0, 3.0],
                percent: None
            })
        );
        assert_eq!(parse("person brightness 55.5"), Ok(Msg::Brightness(55.5)));
        assert_eq!(parse("person power off"), Ok(Msg::Power(Power::Off)));
        assert_eq!(
            parse("person look 999 -5 3 150"),
            Ok(Msg::Look {
                rgb: [255.0, 0.0, 3.0],
                percent: Some(100.0)
            })
        );
    }

    #[test]
    fn anything_else_is_handed_over_as_json() {
        assert_eq!(
            parse(r#"person act {"effect":"rainbow"}"#),
            Ok(Msg::Act(r#"{"effect":"rainbow"}"#.to_string()))
        );
        assert!(parse("person act rainbow").is_err());
    }

    #[test]
    fn agents_and_housekeeping() {
        assert_eq!(parse("agent needs-you"), Ok(Msg::Agent(Agent::NeedsYou)));
        assert!(parse("agent dancing").is_err());
        assert_eq!(parse("status"), Ok(Msg::Status));
        assert_eq!(parse("sleep"), Ok(Msg::Sleep));
    }

    #[test]
    fn bad_input_is_an_error_never_a_panic() {
        for bad in [
            "",
            "person",
            "person look",
            "person look 1 2",
            "person look a b c",
            "person look 1 2 3 x",
            "person look NaN 1 1",
            "frobnicate",
            "person power",
            "person nonsense",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn screen_frames_in_three_forms() {
        assert_eq!(screen_frame(&[1, 2, 3]), Some(([1.0, 2.0, 3.0], 100.0)));
        assert_eq!(screen_frame(&[1, 2, 3, 40]), Some(([1.0, 2.0, 3.0], 40.0)));
        assert_eq!(
            screen_frame(&[1, 2, 3, 0x0f, 0xa0]),
            Some(([1.0, 2.0, 3.0], 40.0))
        );
        assert_eq!(screen_frame(&[1, 2]), None);
    }
}
