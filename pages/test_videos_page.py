"""Tests for the video catalog page builder."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import site_chrome, videos_page


class RenderVideosListTest(unittest.TestCase):
    def test_render_videos_list_embeds_each_video_and_escapes_title(self) -> None:
        result = videos_page.render_videos_list(
            [{"id": 'abc123?feature="test"&safe=yes', "title": '1.0.0 <overview> & "tour"'}]
        )

        self.assertIn(
            'src="https://www.youtube-nocookie.com/embed/abc123?feature=&quot;test&quot;&amp;safe=yes"',
            result,
        )
        escaped_title = "1.0.0 &lt;overview&gt; &amp; &quot;tour&quot;"
        self.assertIn(f'title="{escaped_title}"', result)
        self.assertIn(f"<figcaption>{escaped_title}</figcaption>", result)
        self.assertNotIn("<overview>", result)

    def test_render_videos_list_preserves_input_order_and_card_structure(self) -> None:
        result = videos_page.render_videos_list(
            [
                {"id": "old-video", "title": "Old walkthrough"},
                {"id": "new-video", "title": "New walkthrough"},
            ]
        )

        self.assertLess(result.index("old-video"), result.index("new-video"))
        self.assertEqual(result.count('<figure class="video-card">'), 2)
        self.assertEqual(result.count('loading="lazy"'), 2)
        self.assertEqual(result.count("allowfullscreen"), 2)

    def test_render_videos_list_handles_an_empty_catalog(self) -> None:
        self.assertEqual(videos_page.render_videos_list([]), "")


class VideosPageTest(unittest.TestCase):
    def test_build_videos_page_loads_json_and_replaces_the_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text(
                '[{"id": "first", "title": "First"}, '
                '{"id": "second", "title": "Second"}]', encoding="utf-8"
            )
            (pages_dir / "videos.template.html").write_text(
                "<main>before<!--VIDEOS_LIST-->after</main>", encoding="utf-8"
            )

            with (
                patch.object(videos_page, "PAGES_DIR", pages_dir),
                patch.object(videos_page, "VIDEOS_FILE", videos_file),
            ):
                videos_page.build_videos_page(out_dir)

            output = (out_dir / "videos.html").read_text(encoding="utf-8")
            self.assertNotIn("<!--VIDEOS_LIST-->", output)
            self.assertIn("<main>before", output)
            self.assertIn("after</main>", output)
            self.assertLess(output.index("First"), output.index("Second"))
            self.assertEqual(output.count("youtube-nocookie.com/embed/"), 2)

    def test_build_videos_page_rejects_a_template_without_the_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text("[]", encoding="utf-8")
            (pages_dir / "videos.template.html").write_text(
                "<main>No marker</main>", encoding="utf-8"
            )

            with (
                patch.object(videos_page, "PAGES_DIR", pages_dir),
                patch.object(videos_page, "VIDEOS_FILE", videos_file),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                videos_page.build_videos_page(out_dir)

            self.assertFalse((out_dir / "videos.html").exists())

    def test_build_videos_page_escapes_the_version_in_shared_components(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            includes_dir = pages_dir / "includes"
            out_dir = root / "out"
            includes_dir.mkdir(parents=True)
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text("[]", encoding="utf-8")
            (pages_dir / "videos.template.html").write_text(
                "<!--SITE_HEADER--><main><!--VIDEOS_LIST--></main><!--SITE_FOOTER-->",
                encoding="utf-8",
            )
            (includes_dir / "header.html").write_text("<header>Videos</header>", encoding="utf-8")
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )

            with (
                patch.object(videos_page, "PAGES_DIR", pages_dir),
                patch.object(videos_page, "VIDEOS_FILE", videos_file),
                patch.object(site_chrome, "INCLUDES_DIR", includes_dir),
                patch.object(site_chrome, "render_funding_links", return_value=""),
            ):
                videos_page.build_videos_page(out_dir, 'v2<&"')

            output = (out_dir / "videos.html").read_text(encoding="utf-8")

        self.assertIn("<header>Videos</header>", output)
        self.assertIn("<footer>v2&lt;&amp;&quot;</footer>", output)


class VideoCatalogTest(unittest.TestCase):
    def test_video_catalog_has_unique_nonempty_ids_and_titles(self) -> None:
        videos = videos_page.json.loads(videos_page.VIDEOS_FILE.read_text(encoding="utf-8"))
        ids = [video["id"] for video in videos]
        titles = [video["title"] for video in videos]

        self.assertTrue(videos)
        self.assertEqual(len(ids), len(set(ids)), "YouTube video IDs must be unique")
        self.assertEqual(len(titles), len(set(titles)), "YouTube video titles must be unique")
        for video in videos:
            with self.subTest(video=video):
                self.assertTrue(video["id"].strip())
                self.assertTrue(video["title"].strip())
