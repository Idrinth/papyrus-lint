//! Recognizes CreationKit-generated "Fragment Code" blocks so formatting
//! lints/fixes leave the parts CreationKit expects untouched.
//!
//! CreationKit writes fragment scripts (quest/dialogue/scene fragments)
//! with a `;BEGIN FRAGMENT CODE ... ;END FRAGMENT CODE` comment block
//! wrapping boilerplate it manages itself (fragment headers, the
//! generated function signature, `EndFunction`, and the markers
//! themselves). The only part of that block a user actually edits -- and
//! the only part it's safe to reformat -- is the script code between a
//! `;BEGIN CODE`/`;END CODE` pair. Reformatting anything else in the block
//! (even something as small as re-indenting a line or adding a trailing
//! semicolon to a comment) makes CreationKit fail to recognize the
//! fragment.

/// Returns, for each 1-indexed line of `source`, whether it falls inside a
/// `;BEGIN FRAGMENT CODE`/`;END FRAGMENT CODE` block but outside any
/// `;BEGIN CODE`/`;END CODE` pair nested within it. Formatting lints/fixes
/// must leave lines marked `true` completely untouched. Index `0` is
/// unused (always `false`); the returned vector has
/// `source.lines().count() + 1` entries.
pub fn protected_lines(source: &str) -> Vec<bool> {
    #[derive(PartialEq, Eq)]
    enum State {
        Outside,
        Wrapper,
        Code,
    }

    let mut state = State::Outside;
    let mut protected = vec![false; source.lines().count() + 1];

    for (index, line) in source.lines().enumerate() {
        let marker = line.trim_start().to_ascii_uppercase();

        let is_marker_line = if marker.starts_with(";BEGIN FRAGMENT CODE") {
            state = State::Wrapper;
            true
        } else if marker.starts_with(";END FRAGMENT CODE") {
            let was_inside = state != State::Outside;
            state = State::Outside;
            was_inside
        } else if state == State::Wrapper && marker.starts_with(";BEGIN CODE") {
            state = State::Code;
            true
        } else if state == State::Code && marker.starts_with(";END CODE") {
            state = State::Wrapper;
            true
        } else {
            false
        };

        protected[index + 1] = is_marker_line || state == State::Wrapper;
    }

    protected
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAGMENT: &str = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
;NEXT FRAGMENT INDEX 0
Scriptname IDR__TIF__05000235 Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
akSpeaker.RemoveItem(idrinthAlyienethMikaelsSong, 1, false, PlayerRef)
PlayerRef.RemoveItem(Gold001, 5, false, akSpeaker)
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment
Actor Property PlayerRef  Auto
";

    #[test]
    fn protects_wrapper_lines_but_not_code_body() {
        let protected = protected_lines(FRAGMENT);

        // Wrapper header, script line, fragment header, signature, local
        // var, and both `;BEGIN CODE`/`;END CODE` markers are protected.
        for line in [1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15] {
            assert!(protected[line], "expected line {line} to be protected");
        }
        // The actual code body between BEGIN CODE/END CODE is not.
        assert!(!protected[9]);
        assert!(!protected[10]);
    }

    #[test]
    fn lines_outside_the_fragment_block_are_never_protected() {
        let protected = protected_lines(FRAGMENT);
        let last = FRAGMENT.lines().count();
        assert!(!protected[last]);
    }

    #[test]
    fn ordinary_scripts_have_no_protected_lines() {
        let source = "ScriptName Example\nFunction Run()\nEndFunction\n";
        assert!(protected_lines(source).iter().all(|&p| !p));
    }

    #[test]
    fn begin_code_outside_a_wrapper_is_not_treated_as_a_marker() {
        let source = ";BEGIN CODE\nInt x = 1\n;END CODE\n";
        assert!(protected_lines(source).iter().all(|&p| !p));
    }

    #[test]
    fn marker_matching_is_case_insensitive_and_allows_indentation() {
        let source = "  ;begin fragment code\nwrapper\n\t;begin code\nbody\n  ;end code\nwrapper\n ;end fragment code\noutside\n";
        let protected = protected_lines(source);

        for line in [1, 2, 3, 5, 6, 7] {
            assert!(protected[line], "expected line {line} to be protected");
        }
        for line in [4, 8] {
            assert!(!protected[line], "expected line {line} to be editable");
        }
    }

    #[test]
    fn unmatched_end_markers_do_not_protect_ordinary_source() {
        let source = ";END CODE\nInt x = 1\n;END FRAGMENT CODE\nInt y = 2\n";
        assert!(protected_lines(source).iter().all(|&line| !line));
    }

    #[test]
    fn unterminated_wrapper_protects_every_remaining_line() {
        let protected = protected_lines("outside\n;BEGIN FRAGMENT CODE\nwrapper\n");
        assert_eq!(protected, vec![false, false, true, true]);
    }

    #[test]
    fn unterminated_code_section_leaves_remaining_lines_editable() {
        let protected =
            protected_lines(";BEGIN FRAGMENT CODE\nwrapper\n;BEGIN CODE\nbody\nstill body\n");

        assert_eq!(protected, vec![false, true, true, true, false, false]);
    }

    #[test]
    fn supports_multiple_fragment_blocks() {
        let source = ";BEGIN FRAGMENT CODE\none\n;END FRAGMENT CODE\nbetween\n;BEGIN FRAGMENT CODE\ntwo\n;END FRAGMENT CODE\n";
        let protected = protected_lines(source);

        for line in [1, 2, 3, 5, 6, 7] {
            assert!(protected[line], "expected line {line} to be protected");
        }
        assert!(!protected[4]);
    }

    #[test]
    fn marker_prefixes_match_creation_kit_comment_suffixes() {
        let source = ";BEGIN FRAGMENT CODE generated metadata\nwrapper\n;END FRAGMENT CODE generated metadata\noutside\n";

        assert_eq!(
            protected_lines(source),
            vec![false, true, true, true, false]
        );
    }
}
