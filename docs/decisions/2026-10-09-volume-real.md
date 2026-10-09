# 2026-10-09 — volume-real: cot `volume` co that la mot cot so khong, va ho
# "volume" co that la chua tung duoc do?

## Gia thuyet (mot cau)

Ho co che doc `Bar::volume` (`volume-thrust`, `volman-box`, `rsi-reversal-vol`,
`vwap-fade`) chua tung duoc do tren mot feed CO volume, vi moi cua so 16 nam cua
ho so la Dukascopy va cot `volume` cua Dukascopy la **0 tren moi dong**, nen dieu
kien vao khong bao gio thoa — tuc "0-4 lenh" la tieng cua mot cot so khong,
khong phai tieng cua mot co che khong co tin hieu.

## Falsifier — cu the va ban duoc (chu may khai, toi giu nguyen)

Gia thuyet sup o **tien kiem**, va toi tieu **0 o cong**, neu:

- **F1** — volume Dukascopy **khong** phai 0 tren toan bo cot (phep do 50.000
  mau cua chu may sai), **hoac**
- **F2** — cac co che volume **khong** doc cot do theo cach da doan (`mean` cua
  cot 0 chan moi lenh), **hoac**
- **F3** (he qua truc tiep cua ket luan, nen toi khai no la chan thu ba) —
  cac lan chay da cong bo cho **0-4 lenh** KHONG phai tren feed Dukascopy; tuc
  ho volume **da** duoc do tren mot feed co volume roi.

F3 la chan quyet dinh: cau "chua tung duoc do" la tien de duy nhat lam cho viec
do lai dang gia. Neu header receipt in `--market=btc` hay `--market=xauusd` thi
ho da duoc do, va job nay het ly do ton tai.

## Da phep thu — dem O TRUOC

Tien kiem (khong phai o cong, khong tieu gi):

1. **24 file parquet** trong `data/bars/`: dem **toan bo** cot `volume`, tach
   rieng `NULL` / `== 0` / `> 0` / `< 0`, kem min/trung vi/max cua phan duong
   va khoang thoi gian. Khong lay mau.
2. **Doc ma**, 4 file: `volume_thrust.rs`, `volman_box.rs`, `rsi_reversal_vol.rs`,
   `vwap_fade.rs` + ham `vwap()` trong `fd-indicators/src/lib.rs`. Cau hoi:
   `None` xu ly sao, `0.0` xu ly sao, co cho nao chia cho 0, va nguong lay tu dau.
3. **Truy lai cac lan chay da cong bo**: `grep` moi receipt/toml cua bon co che
   do, doc dong `$ ...search.exe --market=` va `bars:` cua tung cai.

O cong khai truoc: **0 neu bat ky chan falsifier nao ban**; neu khong ban thi
toi dang ky rieng mot lan nua truoc khi tieu.

## Cach doc

- `null` != `0` != `[]` (§8). Mot cot NULL va mot cot 0 la **hai khuyet diem
  khac nhau**: cot NULL la "nguon khong cong bo", cot 0 la "nguon cong bo mot
  so khong". Toi bao rieng tung loai, theo tung file.
- **"volume" cua feed sang la TICK VOLUME** (so lan gia doi), khong phai khoi
  luong giao dich. `fd-core/src/market.rs` khai `BarSource::Mt5` la "the
  broker's own OHLC with **tick volume**". Chi `BarSource::Binance` la khoi
  luong that. Toi goi dung ten no o moi cho.
- Khong trich mot con so da cong bo nhu mot du kien (phu luc 8 muc II). Con so
  nao toi dung de ket luan thi toi chay lai va trich cua chinh toi.
- Neu phai bao chan PF: `PF_r` canh `PF_usd`, khai don vi; `E = total_r/n`,
  khong doc `expectancy` 3 chu so (phu luc 8 muc IV); drawdown USD canh moi so
  loi nhuan; `_pct > 100%` ⇒ dong da chay.
