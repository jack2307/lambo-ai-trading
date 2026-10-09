# Receipt 2026-10-09 volume-real

Tien kiem (python, doc truc tiep cot parquet, KHONG lay mau):
  precheck-column-census.txt     24 file data/bars, toan bo cot volume: NULL / 0 / >0 / <0
  precheck-column-shape.txt      hinh dang cot theo nam: p10/p50/p90/p99, p99/p50, cv
  precheck-thrust-rate.txt       dieu kien vao volume-thrust dem truc tiep tren cot, theo nam
  precheck-xauusd-seam.txt       XAUUSD-15m thang qua thang -> vet noi 2023-12
  precheck-split-arithmetic.txt  co cach chia thoi gian nao cho ca hai nua >= 40 tin hieu

Chay lai cac dong DA CONG BO, binary fd-stop-width (lbar_line + drawdown), --exit-mix:
  rerun-gold-{is,oos}-{noguards,guards}.txt   xauusd:15m, batch 2026-09-13-volume-thrust-gold.toml
  rerun-btc-{is,oos}-{noguards,guards}.txt    btc:15m (BTCUSDT), batch 2026-09-13-volume-thrust.toml

Probe:
  probe-vwap-stretch-{1.5,2.5}.txt  stretchAtr co song khong (8.366 vs 6.382 lenh: co)

O cong MOI tieu: 0. Falsifier ban o tien kiem (chan F2 va F3).
Ket qua doc o docs/decisions/2026-10-09-volume-real-result.md
