import os
import unittest
from unittest import mock

import jobs

GOOD_SIGNUP = """Bugs found:
1. After tapping Create account with the terms unchecked, ticking the box and tapping Create account again does nothing.
2. The welcome message drops the last character of the email (ada@example.co instead of ada@example.com).
Steps:
- open sign-up
- fill the fields
- tap Create account
"""
GOOD_SHOP = """1. Decrease quantity from 3 to 2: the total stays at $36 and does not update.
2. Promo code SAVE10 says 10% off but takes a flat $10.00 off.
- steps: add 3, decrease, apply code
- compare totals
- done
"""


class Rubric(unittest.TestCase):
    def test_a_report_with_both_bugs_and_steps_matches(self):
        for task, text in (("qa-signup", GOOD_SIGNUP), ("qa-shop", GOOD_SHOP)):
            self.assertTrue(all(jobs.found_bugs(text, jobs.BUGS[task]).values()), task)
            self.assertTrue(jobs.has_steps(text))

    def test_vague_or_partial_reports_do_not(self):
        vague = "The sign-up flow works but the button feels slow. The layout is a bit cramped."
        self.assertFalse(any(jobs.found_bugs(vague, jobs.BUGS["qa-signup"]).values()))
        half = "The welcome message drops the last character of the email."
        got = jobs.found_bugs(half, jobs.BUGS["qa-signup"])
        self.assertEqual(sorted(k for k, v in got.items() if v), ["welcome-drops-last-char-of-email"])
        self.assertFalse(any(jobs.found_bugs("All good, no problems, total is right, promo works.", jobs.BUGS["qa-shop"]).values()))
        self.assertFalse(jobs.has_steps("it is broken"))
        hedged = "**Possible bug: the first submit after ticking the box may not register**\n1. Tick the box.\n- Actual: the message stayed. A second tap produced the welcome."
        self.assertTrue(jobs.found_bugs(hedged, jobs.BUGS["qa-signup"])["create-button-dead-after-an-error"])


class Checks(unittest.TestCase):
    def test_a_bug_described_over_several_lines_under_one_heading_is_found(self):
        report = (
            "**Bug 2: Total does not update when quantity changes**\n"
            "1. Tap Decrease quantity once.\n"
            "- Actual: Total stays at $38.00.\n"
            "\n**Bug 3: Quantity floor is fine**\nWorks as expected.\n"
        )
        got = jobs.found_bugs(report, jobs.BUGS["qa-shop"])
        self.assertTrue(got["total-stale-after-decrease"])
        self.assertFalse(got["save10-takes-a-flat-10-off"])

    def test_a_decimal_point_does_not_split_a_sentence_of_the_report(self):
        got = jobs.found_bugs(
            "Quantity 3 to 2: the total stays $36.00 and is not updated after Decrease.",
            jobs.BUGS["qa-shop"],
        )
        self.assertTrue(got["total-stale-after-decrease"])

    def test_locker_passes_on_the_code_and_fails_on_a_wrong_number_or_shell_use(self):
        with mock.patch.object(jobs, "relay_code", return_value="4829173"):
            ok = ([{"_id": "1"}], ["4829173"])
            with mock.patch.object(jobs, "contact_phones", return_value=ok):
                self.assertEqual(jobs.check_locker("Locker 7", True), [])
                with mock.patch.object(jobs, "shell_used", return_value="mcp__phonedriver__shell {}"):
                    self.assertEqual(len(jobs.check_locker("Locker 7", True)), 1)
                    self.assertEqual(jobs.check_locker("Locker 9", False), [])
            with mock.patch.object(jobs, "contact_phones", return_value=([{"_id": "1"}], ["4829137"])):
                self.assertIn("4829137", jobs.check_locker("Locker 7", True)[0])
            with mock.patch.object(jobs, "contact_phones", return_value=([{"_id": "1"}], ["14829173"])):
                self.assertEqual(jobs.check_locker("Locker 7", True), [])
            with mock.patch.object(jobs, "contact_phones", return_value=([], [])):
                self.assertIn("0 contacts", jobs.check_locker("Locker 7", True)[0])

    def test_clock_settings(self):
        prefs = (
            '<map><string name="snooze_duration">15</string><string name="auto_silence">5</string>'
            '<string name="home_time_zone">Europe/London</string>'
            '<string name="place_timezone_id_0">Asia/Tokyo</string><string name="place_timezone_id_1">Europe/London</string></map>'
        )
        with mock.patch.object(jobs, "su", return_value=prefs):
            self.assertEqual(jobs.check_clock_settings(), [])
        default = '<map><string name="snooze_duration">10</string><string name="auto_silence">10</string><string name="home_time_zone">Europe/London</string></map>'
        with mock.patch.object(jobs, "su", return_value=default):
            bad = jobs.check_clock_settings()
            self.assertEqual(len(bad), 4, bad)

    def test_messages_pin_and_archive(self):
        def rows(pin, arch):
            return [
                {"name": "Alice Chen", "pinned": "0", "archived": "0"},
                {"name": "Bob Park", "pinned": pin, "archived": "0"},
                {"name": "Carol Diaz", "pinned": "0", "archived": arch},
            ]

        with mock.patch.object(jobs, "conversations", return_value=rows("1", "1")):
            self.assertEqual(jobs.check_messages(), [])
        with mock.patch.object(jobs, "conversations", return_value=rows("0", "1")):
            self.assertIn("Bob Park", jobs.check_messages()[0])
        with mock.patch.object(jobs, "conversations", return_value=rows("1", "1")[:2]):
            self.assertIn("0 conversations named Carol Diaz", jobs.check_messages()[0])

    def test_photos_needs_a_new_square_and_the_original(self):
        orig = {"_display_name": "jobs-harbor.png", "width": "1200", "height": "800", "date_added": "100"}
        sq = {"_display_name": "jobs-harbor-edited.jpg", "width": "800", "height": "800", "date_added": "200"}
        wide = dict(sq, width="1200", _display_name="x.jpg")
        with mock.patch.object(jobs, "sh", return_value="150"):
            with mock.patch.object(jobs, "media_images", return_value=[orig, sq]):
                self.assertEqual(jobs.check_photos(), [])
            with mock.patch.object(jobs, "media_images", return_value=[orig, wide]):
                self.assertIn("no new square", jobs.check_photos()[0])
            with mock.patch.object(jobs, "media_images", return_value=[sq]):
                self.assertIn("original", jobs.check_photos()[0])

    def test_wakelock_only_the_wakelock_holder_may_be_restricted(self):
        def run(now, base="com.old"):
            def fake(cmd):
                return base if cmd.startswith("cat") else ""
            with mock.patch.object(jobs, "sh", side_effect=fake), mock.patch.object(jobs, "appops_ignored", return_value=now):
                return jobs.check_wakelock()

        self.assertEqual(run({"com.old", jobs.TESTBED}), [])
        self.assertIn("not restricted", run({"com.old"})[0])
        self.assertIn("other apps", run({"com.old", jobs.TESTBED, "com.chrome"})[0])

    def test_signup_log_must_show_the_error_path_and_the_dead_tap(self):
        full = [
            {"ev": "signup_tap", "n": 1, "accepted": False, "error": "Please accept the terms"},
            {"ev": "signup_tap", "n": 2, "accepted": False, "ignored": True},
            {"ev": "signup_tap", "n": 3, "accepted": True},
        ]
        with mock.patch.dict(os.environ, {"ANSWER_FILE": "/dev/null"}):
            with mock.patch.object(jobs, "answer", return_value=GOOD_SIGNUP):
                with mock.patch.object(jobs, "log_events", return_value=full):
                    self.assertEqual(jobs.check_qa_signup(), [])
                with mock.patch.object(jobs, "log_events", return_value=full[2:]):
                    self.assertEqual(len(jobs.check_qa_signup()), 2)


class Seeds(unittest.TestCase):
    def test_the_generated_png_has_the_requested_size(self):
        data = jobs.png(1200, 800)
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(int.from_bytes(data[16:20], "big"), 1200)
        self.assertEqual(int.from_bytes(data[20:24], "big"), 800)


if __name__ == "__main__":
    unittest.main()
