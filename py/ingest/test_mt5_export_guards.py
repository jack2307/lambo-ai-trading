"""Tests for the two guards added to `mt5_export.py` on 2026-10-11.

    python py/ingest/test_mt5_export_guards.py          # or: python -m unittest ...

Stdlib `unittest` and nothing else: this tree has no pytest, and a guard that
can only be checked after installing a test runner is a guard that does not get
checked. Importing `mt5_export` imports `MetaTrader5`, which does NOT connect to
anything — the connection is `initialize()`, and nothing here calls it.

What these tests are for
------------------------
Two defects were found by reading `mt5_export.py` before running it:

1. `symbol_select` was called UNCONDITIONALLY on every symbol, while
   `py/live/spread_log.py` states in its doc comment that for this account the
   call is deliberately never made. Two scripts in one tree, opposite rules,
   same MT5 call.
2. `--days` defaults to `None`, which walks ALL of history, while
   `--since-stored` defaults to off — and `write_bars` replaces the whole file
   from a read-modify-write merge. So one hand-typed command pointed at a store
   rewrites the value of every bar in it. That is the shape that cost this
   dossier 25 of 34 published rows (`AGENT-BRIEF-ADDENDUM-11` section VII).

And the thing being guarded against in BOTH cases is not a wrong answer, it is
a SILENT one. So the tests below do not only check what the decision functions
return. They check that the call site obeys them (`SymbolSelectTripwire`), that
the real parser rejects a misspelled or abbreviated flag instead of quietly
matching it, and that the source holds exactly one `symbol_select` call site —
the Python counterpart of the source-scanning flag test on the Rust side.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import mt5_export  # noqa: E402


class Info:
    """The two fields of `symbol_info` that the decision reads."""

    def __init__(self, visible):
        self.visible = visible


def quiet(*_args):
    pass


class TestSymbolSelectPlan(unittest.TestCase):
    """The decision itself: four states x two flag values."""

    def test_visible_proceeds_and_says_it_did_not_call(self):
        action, reason = mt5_export.symbol_select_plan(Info(True), allow_select=False)
        self.assertEqual(action, "proceed")
        self.assertIn("NOT called", reason)

    def test_visible_proceeds_even_WITH_the_flag(self):
        """The flag is permission, not an instruction.

        A symbol already in Market Watch cannot be made readable by selecting
        it, so there is nothing for the call to do and it is not made. If this
        ever flipped, passing the flag for one unreadable symbol would start
        calling `symbol_select` on every other symbol in the same run.
        """
        action, reason = mt5_export.symbol_select_plan(Info(True), allow_select=True)
        self.assertEqual(action, "proceed")
        self.assertIn("NOT called", reason)

    def test_not_visible_without_flag_skips(self):
        action, reason = mt5_export.symbol_select_plan(Info(False), allow_select=False)
        self.assertEqual(action, "skip")
        self.assertIn("--allow-symbol-select", reason)

    def test_not_visible_with_flag_selects(self):
        action, _ = mt5_export.symbol_select_plan(Info(False), allow_select=True)
        self.assertEqual(action, "select")

    def test_info_none_without_flag_skips(self):
        """`None` is the state a NOT-LOGGED-IN terminal produces.

        Measured on 2026-10-11: the running terminal had no account, so this is
        the branch a run would actually have taken. It must skip and say so, not
        reach for `symbol_select` to fix it.
        """
        action, reason = mt5_export.symbol_select_plan(None, allow_select=False)
        self.assertEqual(action, "skip")
        self.assertIn("NOT called", reason)

    def test_info_none_with_flag_selects(self):
        action, _ = mt5_export.symbol_select_plan(None, allow_select=True)
        self.assertEqual(action, "select")

    def test_missing_visible_attribute_is_treated_as_not_visible(self):
        """An object without `visible` must not read as visible.

        `getattr(info, "visible", False)` decides this, and the default matters:
        a `True` default would make an unexpected shape of `symbol_info` silently
        take the permissive branch.
        """
        action, _ = mt5_export.symbol_select_plan(object(), allow_select=False)
        self.assertEqual(action, "skip")


class SymbolSelectTripwire:
    """A stub `MetaTrader5` whose `symbol_select` fails the test if called."""

    def __init__(self, info, called_ok=False):
        self._info = info
        self.called = False
        self._called_ok = called_ok

    def symbol_info(self, _symbol):
        return self._info

    def symbol_select(self, _symbol, _enable):
        self.called = True
        if not self._called_ok:
            raise AssertionError("symbol_select was called on a path that must not call it")
        return True

    def last_error(self):
        return (0, "stub")


class TestCallSiteObeysThePlan(unittest.TestCase):
    """Behaviour, not declaration. The plan could be right and ignored."""

    def test_visible_path_does_not_call_symbol_select(self):
        mt5_stub = SymbolSelectTripwire(Info(True))
        ok, info, action = mt5_export.ensure_market_watch("XAUUSD.sc", False, mt5_stub, quiet)
        self.assertTrue(ok)
        self.assertEqual(action, "proceed")
        self.assertIs(info, mt5_stub.symbol_info("XAUUSD.sc"))
        self.assertFalse(mt5_stub.called, "symbol_select must not be called for a visible symbol")

    def test_visible_path_with_flag_still_does_not_call(self):
        mt5_stub = SymbolSelectTripwire(Info(True))
        ok, _, action = mt5_export.ensure_market_watch("XAUUSD.sc", True, mt5_stub, quiet)
        self.assertTrue(ok)
        self.assertEqual(action, "proceed")
        self.assertFalse(mt5_stub.called)

    def test_flag_off_and_not_visible_reports_and_stops_without_calling(self):
        """The branch the owner's decision names: refuse, do not call."""
        mt5_stub = SymbolSelectTripwire(Info(False))
        out = io.StringIO()
        ok, info, action = mt5_export.ensure_market_watch(
            "XAUUSD.sc", False, mt5_stub, lambda *a: print(*a, file=out))
        self.assertFalse(ok)
        self.assertIsNone(info)
        self.assertEqual(action, "skip")
        self.assertFalse(mt5_stub.called, "symbol_select must not be called with the flag off")
        self.assertIn("--allow-symbol-select", out.getvalue(),
                      "a refusal must say what it refused and how to permit it")

    def test_info_none_and_flag_off_does_not_call(self):
        mt5_stub = SymbolSelectTripwire(None)
        ok, _, action = mt5_export.ensure_market_watch("XAUUSD.sc", False, mt5_stub, quiet)
        self.assertFalse(ok)
        self.assertEqual(action, "skip")
        self.assertFalse(mt5_stub.called)

    def test_flag_on_and_not_visible_DOES_call(self):
        """The positive control: show the call site can still reach the call.

        Without this, every assertion above would also pass on a build where
        `symbol_select` had simply been deleted — and a guard that works by
        removing the capability is not the guard that was asked for.
        """
        mt5_stub = SymbolSelectTripwire(Info(False), called_ok=True)
        ok, info, action = mt5_export.ensure_market_watch("XAUUSD.sc", True, mt5_stub, quiet)
        self.assertTrue(mt5_stub.called, "with the flag on and visible == False it MUST call")
        self.assertEqual(action, "select")
        self.assertTrue(ok)
        self.assertIsNotNone(info)


class TestRewritePlan(unittest.TestCase):
    def test_empty_store_is_free(self):
        ok, why = mt5_export.rewrite_plan(0, since_stored=False, rewrite_existing=False)
        self.assertTrue(ok)
        self.assertIn("nothing stored", why)

    def test_existing_store_is_refused_by_default(self):
        ok, why = mt5_export.rewrite_plan(100586, since_stored=False, rewrite_existing=False)
        self.assertFalse(ok)
        self.assertIn("REFUSED", why)
        self.assertIn("100586", why, "the refusal must name how many bars were at risk")
        self.assertIn("--since-stored", why)
        self.assertIn("--rewrite-existing", why)

    def test_since_stored_is_a_way_through(self):
        ok, _ = mt5_export.rewrite_plan(100586, since_stored=True, rewrite_existing=False)
        self.assertTrue(ok)

    def test_rewrite_existing_is_a_way_through_and_says_what_it_means(self):
        ok, why = mt5_export.rewrite_plan(100586, since_stored=False, rewrite_existing=True)
        self.assertTrue(ok)
        self.assertIn("rewrite", why)

    def test_days_is_structurally_not_a_way_through(self):
        """`--days` must not be able to unlock this, so it is not a parameter.

        A bounded walk still rewrites every stored bar inside its bound:
        `--days 400` against this store would rewrite 400 days of it. The
        cleanest way to stop `--days` from being mistaken for a safety bound is
        for the latch not to be able to see it, and this test pins that.
        """
        import inspect
        params = list(inspect.signature(mt5_export.rewrite_plan).parameters)
        self.assertEqual(params, ["stored_count", "since_stored", "rewrite_existing"])
        self.assertNotIn("days", params)


class TestFlagsCannotBeSwallowed(unittest.TestCase):
    """The real parser, not a copy of it."""

    def parse(self, argv):
        return mt5_export.parse_args(argv)

    def expect_exit(self, argv):
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit) as cm:
            self.parse(argv)
        self.assertNotEqual(cm.exception.code, 0)
        return err.getvalue()

    def test_defaults_are_the_safe_ones(self):
        _, args = self.parse([])
        self.assertFalse(args.allow_symbol_select)
        self.assertFalse(args.rewrite_existing)
        self.assertFalse(args.since_stored)
        self.assertIsNone(args.days, "--days defaults to None, which is ALL of history")

    def test_both_new_flags_actually_land(self):
        _, args = self.parse(["--allow-symbol-select", "--rewrite-existing"])
        self.assertTrue(args.allow_symbol_select)
        self.assertTrue(args.rewrite_existing)

    def test_abbreviation_is_an_error_not_a_silent_match(self):
        """`allow_abbrev=False`. This is the whole point of that setting.

        With abbreviation on, `--allow-sym` would set `allow_symbol_select` and
        the caller would never learn they had written something else.
        """
        for typo in ("--allow-sym", "--allow", "--rewrite", "--since"):
            with self.subTest(typo=typo):
                self.assertIn("unrecognized", self.expect_exit([typo]).lower())

    def test_a_plausible_wrong_spelling_is_an_error(self):
        for wrong in ("--no-symbol-select", "--no-select", "--allow_symbol_select",
                      "--overwrite-existing"):
            with self.subTest(wrong=wrong):
                self.expect_exit([wrong])

    def test_garbage_days_is_a_hard_error(self):
        for bad in ("0", "-1", "abc"):
            with self.subTest(days=bad):
                self.expect_exit(["--days", bad])

    def test_good_days_still_parses(self):
        _, args = self.parse(["--days", "400"])
        self.assertEqual(args.days, 400)

    def test_flag_table_reports_origin(self):
        """A flag table that said "default" for everything would be useless."""
        ap, args = self.parse(["--since-stored"])
        rows = {name: (value, origin) for name, value, origin in
                mt5_export.flag_table(ap, args, ["--since-stored"])}
        self.assertEqual(rows["--since-stored"], (True, "command line"))
        self.assertEqual(rows["--rewrite-existing"], (False, "default"))
        self.assertIn("--allow-symbol-select", rows,
                      "every flag must appear, or the table hides the one that matters")


class TestSourceHoldsOneCallSite(unittest.TestCase):
    """The counterpart of the Rust source-scanning flag test.

    `ensure_market_watch` is only a meaningful guard while it is the ONLY place
    that calls `symbol_select`. A second call site added later would pass every
    other test in this file.
    """

    def test_exactly_one_symbol_select_call_site_and_it_is_in_ensure_market_watch(self):
        """Scanned with `ast`, not with `in`.

        A substring scan found two hits and only one was a call: the other was
        the word `symbol_select()` inside the `--allow-symbol-select` help
        string. A test that counts mentions rather than calls fails when the
        documentation improves and passes when a second call is added inside a
        comment-free one-liner, which is backwards. The parser knows which is
        which, so ask it.
        """
        import ast
        path = os.path.join(os.path.dirname(os.path.abspath(mt5_export.__file__)),
                            "mt5_export.py")
        with open(path, encoding="utf-8") as fh:
            tree = ast.parse(fh.read())
        sites = []  # (lineno, enclosing function name)
        for node in ast.walk(tree):
            if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                continue
            for inner in ast.walk(node):
                if (isinstance(inner, ast.Call)
                        and isinstance(inner.func, ast.Attribute)
                        and inner.func.attr == "symbol_select"):
                    sites.append((inner.lineno, node.name))
        # `ast.walk` visits nested functions too; keep the innermost owner.
        by_line = {}
        for lineno, fname in sites:
            by_line.setdefault(lineno, []).append(fname)
        self.assertEqual(len(by_line), 1,
                         f"expected one symbol_select call site, found {len(by_line)} "
                         f"at lines {sorted(by_line)}")
        owners = next(iter(by_line.values()))
        self.assertIn("ensure_market_watch", owners,
                      f"the call site is in {owners}, not ensure_market_watch")

    def test_the_standing_rule_is_quoted_in_the_module_doc(self):
        """So the next reader does not have to find `spread_log.py` to know why."""
        self.assertIn("deliberately NOT called", mt5_export.__doc__)


class StubTerminal:
    """Enough of `MetaTrader5` to drive `main()` with no terminal and no account.

    `copy_rates_range` answers with plain dicts. The real module returns a numpy
    structured array, and the code reads it by field name only (`r["time"]`,
    `r["open"]`, ...), so dicts exercise the same lines. `len(rates)` drives the
    end-of-history test either way.
    """

    TIMEFRAME_M15 = 15

    def __init__(self, bars, visible=True):
        self.bars = bars           # list of (server_epoch, o, h, l, c, tickvol)
        self.visible = visible
        self.select_called = False
        self.ranges_asked = 0

    # -- connection ---------------------------------------------------------
    def initialize(self, path=None):
        return True

    def shutdown(self):
        return True

    def last_error(self):
        return (0, "stub")

    def terminal_info(self):
        return type("T", (), {"name": "stub", "build": 0, "trade_allowed": False})()

    def account_info(self):
        return type("A", (), {"login": 33705331, "server": "VantageMarkets-Live 21"})()

    # -- symbols ------------------------------------------------------------
    def symbol_info(self, _symbol):
        return type("I", (), {"visible": self.visible, "digits": 2,
                              "spread": 22, "trade_contract_size": 100.0})()

    def symbol_select(self, _symbol, _enable):
        self.select_called = True
        return True

    # -- history ------------------------------------------------------------
    def copy_rates_range(self, _symbol, _tf, start, end):
        self.ranges_asked += 1
        lo, hi = start.timestamp(), end.timestamp()
        return [{"time": t, "open": o, "high": h, "low": lo_, "close": c, "tick_volume": v}
                for (t, o, h, lo_, c, v) in self.bars if lo <= t < hi]


class TestLatchEndToEnd(unittest.TestCase):
    """Drive `main()`. The unit tests above check the decision; this checks the RUN.

    `AGENT-BRIEF-ADDENDUM-11` section VI: `[run] max_hold_ms` in a batch file was
    accepted without complaint while the cap stayed at four hours, which is a
    flag that parses, reads back correctly, and never reaches the thing it names.
    The only way to tell that apart from a working flag is to measure it where it
    lands. So these tests set the flag, run the whole of `main()`, and read the
    exit code and the file on disk.
    """

    def setUp(self):
        import tempfile
        self.tmp = tempfile.mkdtemp(prefix="mt5export_latch_")
        self.out = os.path.join(self.tmp, "bars")
        self.path = os.path.join(self.out, "XAUUSD-15m.parquet")
        self.real_mt5 = mt5_export.mt5
        self.real_argv = sys.argv

    def tearDown(self):
        import shutil
        mt5_export.mt5 = self.real_mt5
        sys.argv = self.real_argv
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_main(self, argv, stub):
        mt5_export.mt5 = stub
        sys.argv = ["mt5_export.py"] + argv
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = mt5_export.main()
        return code, out.getvalue()

    def bars_ending(self, last_utc, count):
        """`count` M15 bars ending at `last_utc` (a UTC datetime), server clock.

        The script subtracts the server offset on the way in, so the stub hands
        back server-clock epochs: UTC plus three hours inside New York DST.
        """
        base = int(last_utc.timestamp()) + 3 * 3600
        return [(base - i * 900, 100.0 + i, 101.0 + i, 99.0 + i, 100.5 + i, 10 + i)
                for i in range(count)][::-1]

    def test_fresh_store_is_written_and_symbol_select_is_not_called(self):
        import datetime as dt
        stub = StubTerminal(self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50))
        code, log = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out], stub)
        self.assertEqual(code, 0, log)
        self.assertTrue(os.path.exists(self.path), log)
        self.assertFalse(stub.select_called, "visible symbol must not be selected")
        self.assertIn("nothing stored yet", log)

    def test_second_run_over_an_existing_store_is_REFUSED(self):
        """The trap, closed. Same command twice: the second one must refuse."""
        import datetime as dt
        bars = self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50)
        code, _ = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out],
            StubTerminal(bars))
        self.assertEqual(code, 0)
        import pyarrow.parquet as pq
        before = pq.read_table(self.path).num_rows
        self.assertGreater(before, 0)

        stub2 = StubTerminal(bars)
        code2, log2 = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out], stub2)
        self.assertEqual(code2, 1, f"a run that exported nothing must not report 0:\n{log2}")
        self.assertIn("REFUSED", log2)
        self.assertEqual(stub2.ranges_asked, 0,
                         "a refusal must cost nothing: the terminal must not be asked for bars")
        self.assertEqual(pq.read_table(self.path).num_rows, before,
                         "the refused run must not have touched the file")

    def test_since_stored_gets_through_the_latch_and_appends(self):
        import datetime as dt
        import pyarrow.parquet as pq
        first = dt.datetime(2026, 10, 10, 12, 0, tzinfo=dt.timezone.utc)
        code, _ = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out],
            StubTerminal(self.bars_ending(first, 20)))
        self.assertEqual(code, 0)
        before = pq.read_table(self.path).num_rows

        later = dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc)
        stub2 = StubTerminal(self.bars_ending(later, 140))
        code2, log2 = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--since-stored",
             "--out", self.out], stub2)
        self.assertEqual(code2, 0, log2)
        self.assertIn("--since-stored bounds the pull", log2)
        after = pq.read_table(self.path).num_rows
        self.assertGreater(after, before, f"the tail should have been appended:\n{log2}")

    def test_rewrite_existing_gets_through_the_latch(self):
        import datetime as dt
        bars = self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50)
        self.run_main(["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3",
                       "--out", self.out], StubTerminal(bars))
        code, log = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3",
             "--rewrite-existing", "--out", self.out], StubTerminal(bars))
        self.assertEqual(code, 0, log)
        self.assertIn("--rewrite-existing was passed", log)
        self.assertNotIn("REFUSED", log)

    def test_not_visible_without_the_flag_exports_nothing_and_never_selects(self):
        import datetime as dt
        stub = StubTerminal(
            self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50),
            visible=False)
        code, log = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out], stub)
        self.assertEqual(code, 1, log)
        self.assertFalse(stub.select_called)
        self.assertFalse(os.path.exists(self.path))
        self.assertIn("--allow-symbol-select", log)

    def test_not_visible_WITH_the_flag_selects_and_exports(self):
        """Positive control for the end-to-end path, not just the helper."""
        import datetime as dt
        stub = StubTerminal(
            self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50),
            visible=False)
        code, log = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3",
             "--allow-symbol-select", "--out", self.out], stub)
        self.assertEqual(code, 0, log)
        self.assertTrue(stub.select_called)
        self.assertTrue(os.path.exists(self.path))

    def test_the_flag_table_is_printed_so_a_default_is_visible(self):
        import datetime as dt
        stub = StubTerminal(self.bars_ending(dt.datetime(2026, 10, 11, 12, 0, tzinfo=dt.timezone.utc), 50))
        _, log = self.run_main(
            ["--symbols", "XAUUSD.sc", "--timeframes", "M15", "--days", "3", "--out", self.out], stub)
        self.assertIn("flags in effect", log)
        self.assertIn("--rewrite-existing", log)
        self.assertIn("[default]", log)
        self.assertIn("[command line]", log)


if __name__ == "__main__":
    unittest.main(verbosity=2)
