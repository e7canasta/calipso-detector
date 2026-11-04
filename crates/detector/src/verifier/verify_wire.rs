//! The wire of the slow verifier's protocol (`calipso.verify.v1`): the
//! request's `Calipso-*` headers and its payload of crops, and the reply read
//! back as an answer. The protocol — who is who, the headers, the payload and
//! the reply — is documented in `contract/verify-v1.md`. Nothing here touches
//! a pixel (rule 7): only the shapes the request and the reply take.

use scene_look::{Answer, Question, Ticket};

/// The request's `Calipso-*` headers: which camera and stream ask, about
/// which ticket and question, over which bed (its corners in the crop's
/// pixels), and which sample is looked at. With a burst, two more say how many
/// crops go and how long each one is; one crop goes alone and without them,
/// as before bursts.
pub(crate) fn request_headers(
    camera: &str,
    stream: &str,
    ticket: Ticket,
    question: Question,
    bed: &str,
    jpegs: &[Vec<u8>],
) -> async_nats::HeaderMap {
    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Calipso-Camera", camera);
    headers.insert("Calipso-Question", question.id());
    headers.insert("Calipso-Ticket", ticket.id.to_string().as_str());
    headers.insert("Calipso-Stream", stream);
    headers.insert("Calipso-Bed", bed);
    let looked = ticket.looked;
    let looked = format!(
        "{}:{}:{}",
        looked.session.0, looked.sequence, looked.source_secs.0
    );
    headers.insert("Calipso-Looked", looked.as_str());
    if let Some(lengths) = frame_lengths(jpegs) {
        headers.insert("Calipso-Frames", jpegs.len().to_string().as_str());
        headers.insert("Calipso-Frame-Lengths", lengths.as_str());
    }
    headers
}

/// Each crop's length in bytes, for the `Calipso-Frame-Lengths` header: only
/// when more than one goes. One source of truth for the lengths, shared by
/// the payload and the headers.
fn frame_lengths(jpegs: &[Vec<u8>]) -> Option<String> {
    (jpegs.len() > 1).then(|| {
        jpegs
            .iter()
            .map(|j| j.len().to_string())
            .collect::<Vec<_>>()
            .join(",")
    })
}

/// The payload and the `Calipso-Frame-Lengths` header of a request: the crops
/// one after the other, and each one's length in bytes. One crop goes alone
/// and without the header, as before bursts.
pub(crate) fn frames_payload(jpegs: &[Vec<u8>]) -> (Vec<u8>, Option<String>) {
    (jpegs.concat(), frame_lengths(jpegs))
}

/// What the verifier said, from its reply: `yes`, `no` or anything else, which
/// is unsure. The reply is JSON with an `answer` field, or the bare word.
pub(crate) fn read_answer(reply: &[u8]) -> Answer {
    let text = serde_json::from_slice::<serde_json::Value>(reply)
        .ok()
        .and_then(|v| v.get("answer").and_then(|a| a.as_str()).map(str::to_owned))
        .unwrap_or_else(|| String::from_utf8_lossy(reply).into_owned());
    let word: String = text
        .trim()
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    match word.as_str() {
        "yes" | "si" | "sí" => Answer::Yes,
        "no" => Answer::No,
        _ => Answer::Unsure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene_contract::{SampleRef, SessionId, SourceSecs};

    fn ticket(id: u64) -> Ticket {
        let at = SampleRef {
            session: SessionId(1),
            sequence: 12,
            source_secs: SourceSecs(12.5),
        };
        Ticket {
            id,
            looked: at,
            target: scene_look::Target::Held { since: at },
        }
    }

    fn said(headers: &async_nats::HeaderMap, name: &str) -> Option<String> {
        headers.get(name).map(|v| v.as_str().to_owned())
    }

    #[test]
    fn the_reply_is_read_as_yes_no_or_unsure() {
        assert_eq!(read_answer(br#"{"answer":"yes"}"#), Answer::Yes);
        assert_eq!(read_answer(br#"{"answer":"No."}"#), Answer::No);
        assert_eq!(read_answer(b"Yes, the person is sitting up."), Answer::Yes);
        assert_eq!(read_answer(b"I cannot tell"), Answer::Unsure);
        assert_eq!(read_answer(b""), Answer::Unsure);
    }

    /// The first alphabetic word decides, and the affirmative words are
    /// `yes`, `si` and `sí` (contract/verify-v1.md).
    #[test]
    fn the_first_word_of_the_reply_decides_the_answer() {
        assert_eq!(read_answer(b"si"), Answer::Yes);
        assert_eq!(read_answer(b"s\xC3\xAD, desde aqu\xC3\xAD"), Answer::Yes);
        assert_eq!(read_answer(b"{\"answer\":\"s\xC3\xAD\"}"), Answer::Yes);
        assert_eq!(read_answer(br#"{"answer":"si"}"#), Answer::Yes);
        assert_eq!(read_answer(b"no"), Answer::No);
        assert_eq!(read_answer(b"maybe"), Answer::Unsure);
        assert_eq!(read_answer(b"123 yes"), Answer::Unsure);
    }

    #[test]
    fn a_burst_goes_as_its_crops_in_order_with_their_lengths() {
        let one = frames_payload(&[vec![1, 2]]);
        assert_eq!(one, (vec![1, 2], None));
        let three = frames_payload(&[vec![1, 2], vec![3], vec![4, 5, 6]]);
        assert_eq!(three, (vec![1, 2, 3, 4, 5, 6], Some("2,1,3".to_owned())));
    }

    /// A round trip of the headers: what the request says of itself is what a
    /// reader takes back. One crop goes without the frames' headers.
    #[test]
    fn the_request_headers_name_the_request() {
        let headers = request_headers(
            "cam",
            "calipso.verify.v1",
            ticket(7),
            Question::HeldInBed,
            "2.0,4.0 32.0,4.0 32.0,22.0 2.0,22.0",
            &[vec![1, 2]],
        );
        assert_eq!(said(&headers, "Calipso-Camera").as_deref(), Some("cam"));
        assert_eq!(
            said(&headers, "Calipso-Question").as_deref(),
            Some("held_in_bed@1")
        );
        assert_eq!(said(&headers, "Calipso-Ticket").as_deref(), Some("7"));
        assert_eq!(
            said(&headers, "Calipso-Stream").as_deref(),
            Some("calipso.verify.v1")
        );
        assert_eq!(
            said(&headers, "Calipso-Bed").as_deref(),
            Some("2.0,4.0 32.0,4.0 32.0,22.0 2.0,22.0")
        );
        assert_eq!(
            said(&headers, "Calipso-Looked").as_deref(),
            Some("1:12:12.5")
        );
        assert_eq!(said(&headers, "Calipso-Frames"), None);
        assert_eq!(said(&headers, "Calipso-Frame-Lengths"), None);
    }

    /// A round trip of a burst: the lengths the headers carry are the payload's
    /// crops' own lengths, in their order.
    #[test]
    fn a_burst_request_carries_its_frame_lengths() {
        let jpegs = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
        let headers = request_headers(
            "cam",
            "calipso.verify.v1",
            ticket(8),
            Question::SittingInBed,
            "bed",
            &jpegs,
        );
        assert_eq!(said(&headers, "Calipso-Frames").as_deref(), Some("3"));
        let said = said(&headers, "Calipso-Frame-Lengths").expect("a burst carries its lengths");
        assert_eq!(said, "2,1,3");
        let back: Vec<usize> = said
            .split(',')
            .map(|n| n.parse::<usize>().expect("a length in bytes"))
            .collect();
        assert_eq!(back, jpegs.iter().map(Vec::len).collect::<Vec<_>>());
        let (payload, lengths) = frames_payload(&jpegs);
        assert_eq!(payload.len(), 6);
        assert_eq!(lengths.as_deref(), Some("2,1,3"));
    }
}
