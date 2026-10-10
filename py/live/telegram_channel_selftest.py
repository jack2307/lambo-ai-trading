"""Offline tests for the account-only channel relay."""
import copy
import unittest

import telegram_channel as C


class ChannelTests(unittest.TestCase):
    def setUp(self):
        self.state = {}
        self.sent = []
        self.replies = []
        self.saved = []
        self.b = dict(account="vantage-v10", symbol="XAUUSD.sc", currency="USC",
                      position=None, fills=[])

    def send(self, text, reply_to=None):
        self.sent.append(text)
        self.replies.append(reply_to)
        return 5000 + len(self.sent)

    def relay(self, brokers=None, send=None, open_position=None):
        C.relay([dict(id="xau-stoch", open=open_position,
                     brokers=brokers or [self.b])],
                self.state, "channel", send or self.send,
                lambda: self.saved.append(copy.deepcopy(self.state)))

    def test_baseline_and_only_v10_then_restart(self):
        self.b["fills"] = [dict(entryTime=1, exitTime=2, pnl=100)]
        self.b["position"] = dict(ticket=1, side="LONG", entry_price=4000, sl=3990, tp=4020)
        self.relay()
        self.assertEqual(self.sent, [])
        self.b["position"]["ticket"] = 2
        other = dict(self.b, account="vantage-v12")
        self.relay([other, self.b])
        self.assertEqual(len(self.sent), 1)
        self.assertIn("MUA XAUUSD", self.sent[0])
        self.assertIn("3990", self.sent[0])
        self.assertIn("TP1  <b>4003.30</b> · 81%", self.sent[0])
        self.assertIn("TP5  <b>4020.00</b> · 11%", self.sent[0])
        self.assertIn("Model: <b>Lambo V10</b>", self.sent[0])
        self.state = copy.deepcopy(self.saved[-1])
        self.relay()
        self.assertEqual(len(self.sent), 1)

    def test_tp_milestones_reply_once_to_the_original_signal(self):
        self.relay()
        self.b["position"] = dict(ticket=7, side="LONG", entry_price=4000,
                                  price_now=4004, sl=3990, tp=4021)
        self.relay()
        original = 5001
        self.assertEqual(self.replies, [None])

        self.relay()
        self.assertEqual(self.sent[-1], "✅ TP1 chạm 4003.30 (+0.33R)")
        self.assertEqual(self.replies[-1], original)

        self.b["position"]["price_now"] = 4007
        self.relay()
        self.assertEqual(self.sent[-1], "✅ TP2 chạm 4006.30 (+0.63R)")
        self.assertEqual(self.replies[-1], original)
        count = len(self.sent)
        self.relay()
        self.assertEqual(len(self.sent), count)

    def test_book_extreme_preserves_a_tp_touch_after_price_retraces(self):
        self.relay()
        self.b["position"] = dict(ticket=71, side="LONG", entry_price=4000,
                                  price_now=4002, sl=3990, tp=4021)
        self.relay()
        self.relay(open_position=dict(side="LONG", entry_price=4000,
                                      risk=10, mfe=.4))
        self.assertEqual(self.sent[-1], "✅ TP1 chạm 4003.30 (+0.33R)")
        self.assertEqual(self.replies[-1], 5001)

    def test_lot_table_never_rounds_above_one_percent(self):
        self.assertEqual(C.sized_lot(1000, 6), .01)
        self.assertEqual(C.sized_lot(10000, 9.33), .10)
        self.assertIsNone(C.sized_lot(500, 20))

    def test_below_minimum_lot_does_not_create_an_html_tag(self):
        shape = C.trade_shape(
            "xau-stoch", self.b,
            dict(side="SHORT", entry_price=4166.03, sl=4177.86, tp=4147.86),
        )
        message = C.signal_message(self.b, shape)
        self.assertNotIn("<0.01", message)
        self.assertIn("dưới 0.01", message)

    def test_close_matching_uses_open_time_and_lots_before_nearest_entry(self):
        signals = {
            "old": dict(side="LONG", entry=4000.2, opened_at=100, lots=.2,
                        closed=False),
            "new": dict(side="LONG", entry=4000, opened_at=200, lots=.4,
                        closed=False),
        }
        fill = dict(direction="LONG", entryPrice=4000, entryTime=100, lots=.2)
        self.assertIs(C.matching_signal(signals, fill), signals["old"])

    def test_close_replies_even_after_tp_and_reports_the_real_loss(self):
        self.relay()
        self.b["position"] = dict(ticket=8, side="SHORT", entry_price=4184.76,
                                  price_now=4181.5, sl=4194.09, tp=4164.69)
        self.relay()
        self.relay()  # notices TP1 on the next poll
        self.b["position"] = None
        self.b["fills"] = [dict(entryTime=10, exitTime=20, direction="SHORT",
                                entryPrice=4184.76, exitPrice=4194.23, lots=.35,
                                pnl=-350.35, exitReason="[sl 4194.09]")]
        self.relay()
        self.assertIn("🏁 Đóng 4194.23 · -1.02R (đã chạm TP1)", self.sent[-1])
        self.assertIn("P&amp;L: <b>-350.35 USC</b>", self.sent[-1])
        self.assertEqual(self.replies[-1], 5001)

    def test_failed_send_retries_and_new_close_at_same_list_length(self):
        """A failed send must not mark the fill seen, so the next cycle retries.

        Re-aimed 2026-10-10: the orphan path no longer sends at all, so this
        property is now exercised where it still applies -- on a MATCHED close.
        """
        self.b["position"] = dict(ticket=1, side="SHORT", entry_price=4000,
                                  sl=4010, tp=3980, opened_at=1, lots=0.5)
        self.relay()                                   # seed, says nothing
        self.b["position"] = dict(self.b["position"], ticket=2, opened_at=3)
        self.relay()                                   # announces the open
        self.assertEqual(len(self.sent), 1)
        self.b["position"] = None
        self.b["fills"] = [dict(entryTime=3, exitTime=4, pnl=-12.5,
                                direction="SHORT", lots=0.5,
                                entryPrice=4000, exitPrice=4010)]

        def fail(_, reply_to=None):
            raise RuntimeError("unreachable")

        with self.assertRaises(RuntimeError):
            self.relay(send=fail)
        self.relay()
        self.assertEqual(len(self.sent), 2)
        self.assertIn("-12.50 USC", self.sent[1])
        self.relay()
        self.assertEqual(len(self.sent), 2)

    def test_other_accounts_and_paper_do_not_publish(self):
        self.relay([dict(self.b, account="vantage-v12")])
        self.b["position"] = dict(ticket=9, side="LONG")
        self.relay([dict(self.b, account="vantage-v12")])
        self.assertEqual(self.sent, [])

    def test_html_and_missing_values(self):
        """Re-aimed 2026-10-10 onto the two messages that still exist."""
        self.b["symbol"] = "<symbol>"
        self.b["position"] = dict(ticket=1, side="SHORT", entry_price=4000,
                                  sl=4010, tp=3980, opened_at=1, lots=0.5)
        self.relay()                                   # seed
        self.b["position"] = dict(self.b["position"], ticket=2, opened_at=3)
        self.relay()
        self.assertIn("&lt;symbol&gt;", self.sent[0])
        self.b["position"] = None
        self.b["fills"] = [dict(entryTime=3, exitTime=4, direction="SHORT",
                                lots=0.5, entryPrice=4000, exitPrice=None,
                                pnl=None)]
        self.relay()
        self.assertIn("? USC", self.sent[1])
        self.assertIn("?R", self.sent[1])

    def test_an_orphan_close_is_recorded_but_never_broadcast(self):
        """The 2026-10-10 flood: 755 fills against 16 announced signals.

        A close whose open this channel never announced must leave no message.
        Before the fix every one of them printed a bare "DONG" line.
        """
        self.b["position"] = dict(ticket=1, side="LONG", entry_price=4000, sl=3990, tp=4020)
        self.relay()                      # seeds the book, says nothing
        self.assertEqual(self.sent, [])
        # A pile of closes arrives with no signal ever posted for them.
        self.b["position"] = None
        self.b["fills"] = [dict(entryTime=10 + i, exitTime=20 + i, direction="LONG",
                                lots=0.5, entryPrice=4100 + i, exitPrice=4090 + i,
                                pnl=-124.8, exitReason="[sl 4090]") for i in range(30)]
        self.relay()
        self.assertEqual(self.sent, [], "orphan closes must not be broadcast")
        # ...and they are recorded, so a later cycle cannot replay them.
        self.relay()
        self.assertEqual(self.sent, [])
        book = self.state["v10_channels"]["channel"]["xau-stoch"]
        self.assertEqual(len(book["fills"]), 30)

    def test_an_open_with_no_ladder_model_is_silent_both_ways(self):
        """`MODELS` carries xau-stoch only; every other run id took the half path."""
        state, sent = {}, []
        broker = dict(account="vantage-v10", symbol="XAUUSD.sc", currency="USC",
                      position=dict(ticket=7, side="LONG", entry_price=4000,
                                    sl=3990, tp=4020),
                      fills=[])
        run = lambda: C.relay([dict(id="btc-macd", open=None, brokers=[broker])],
                              state, "channel", lambda t, reply_to=None: sent.append(t),
                              lambda: None)
        run()                                   # seed
        broker["position"] = dict(broker["position"], ticket=8)
        run()                                   # new ticket, no model
        self.assertEqual(sent, [], "half a signal is worse than silence")
        broker["position"] = None
        broker["fills"] = [dict(entryTime=1, exitTime=2, direction="LONG", lots=0.5,
                                entryPrice=4000, exitPrice=3990, pnl=-50,
                                exitReason="[sl 3990]")]
        run()
        self.assertEqual(sent, [], "and its close stays silent too")


if __name__ == "__main__":
    unittest.main()
