"""Tests for the build-time syntax highlighter."""

from __future__ import annotations

import unittest

from pages import highlighting


class HighlightCodeTest(unittest.TestCase):
    def test_highlight_code_marks_tokens_and_escapes_untrusted_source(self) -> None:
        result = highlighting.highlight_code(
            '{"enabled": true, "count": 12, "unsafe": "<script>"}', "json"
        )

        self.assertIn('<span class="str">&quot;enabled&quot;</span>', result)
        self.assertIn('<span class="kw">true</span>', result)
        self.assertIn('<span class="num">12</span>', result)
        self.assertIn('&lt;script&gt;', result)
        self.assertNotIn('<script>', result)

    def test_highlight_code_supports_aliases_and_plain_text_fallback(self) -> None:
        self.assertIn('<span class="cm"># note</span>', highlighting.highlight_code("# note", "yml"))
        self.assertEqual(highlighting.highlight_code("<unsafe>", "text"), "&lt;unsafe&gt;")

    def test_highlight_code_marks_papyrus_keywords_types_strings_and_comments(self) -> None:
        result = highlighting.highlight_code(
            'ScriptName Example\n\n; note\nFunction Greet(Int aiCount)\n    Debug.Trace("hi")\nEndFunction',
            "papyrus",
        )

        self.assertIn('<span class="kw">ScriptName</span>', result)
        self.assertIn('<span class="kw">Function</span>', result)
        self.assertIn('<span class="kw">EndFunction</span>', result)
        self.assertIn('<span class="ty">Int</span>', result)
        self.assertIn('<span class="cm">; note</span>', result)
        self.assertIn('<span class="str">&quot;hi&quot;</span>', result)

    def test_highlight_code_normalizes_language_names_case_insensitively(self) -> None:
        result = highlighting.highlight_code("if true; then echo 12; fi", "BASH")

        self.assertIn('<span class="kw">if</span>', result)
        self.assertIn('<span class="kw">then</span>', result)
        self.assertIn('<span class="kw">fi</span>', result)

    def test_highlight_code_marks_json_numbers_literals_and_escaped_strings(self) -> None:
        result = highlighting.highlight_code(
            r'{"message": "say \"hello\"", "ratio": -1.25e+3, "missing": null}',
            "json-schema",
        )

        self.assertIn('<span class="str">&quot;message&quot;</span>', result)
        self.assertIn(
            '<span class="str">&quot;say \\&quot;hello\\&quot;&quot;</span>', result
        )
        self.assertIn('<span class="num">-1.25e+3</span>', result)
        self.assertIn('<span class="kw">null</span>', result)

    def test_highlight_code_marks_yaml_tokens_without_coloring_number_like_words(self) -> None:
        result = highlighting.highlight_code(
            'enabled: yes\ncount: -12.5\nrelease: v1.2\nlabel: "safe" # note', "yaml"
        )

        self.assertIn('<span class="kw">yes</span>', result)
        self.assertIn('<span class="num">-12.5</span>', result)
        self.assertIn('<span class="str">&quot;safe&quot;</span>', result)
        self.assertIn('<span class="cm"># note</span>', result)
        self.assertNotIn('v<span class="num">1.2</span>', result)

    def test_highlight_code_marks_shell_and_bbcode_specific_syntax(self) -> None:
        shell = highlighting.highlight_code(
            "for item in 'two words'; do echo \"$item\"; done # note", "sh"
        )
        bbcode = highlighting.highlight_code("[b]Safe[/b] <unsafe>", "bbcode")

        for keyword in ("for", "in", "do", "done"):
            self.assertIn(f'<span class="kw">{keyword}</span>', shell)
        self.assertIn('<span class="str">&#x27;two words&#x27;</span>', shell)
        self.assertIn('<span class="cm"># note</span>', shell)
        self.assertEqual(
            bbcode,
            '<span class="tag">[b]</span>Safe<span class="tag">[/b]</span> &lt;unsafe&gt;',
        )


if __name__ == "__main__":
    unittest.main()
