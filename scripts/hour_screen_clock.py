import numpy as np, pandas as pd

HOUR_MS = 3_600_000
DAY_MS  = 86_400_000
PATH = r'E:/rust/flowdesk/data/bars/XAUDUKA-15m.parquet'

def days_from_civil(y, m, d):
    y = y - 1 if m <= 2 else y
    era = y // 400
    yoe = y - era*400
    mp = m - 3 if m > 2 else m + 9
    doy = (153*mp + 2)//5 + d - 1
    doe = yoe*365 + yoe//4 - yoe//100 + doy
    return era*146097 + doe - 719468

def weekday_of_days(days):
    return (days + 4) % 7

def nth_sunday(year, month, n):
    first = days_from_civil(year, month, 1)
    to_sunday = (7 - weekday_of_days(first)) % 7
    return first + to_sunday + 7*(n-1)

def ny_offset_ms(utc_ms):
    """Exact port of fd_core::clock::new_york_offset_ms (US post-2007 rule)."""
    utc_ms = np.asarray(utc_ms, dtype=np.int64)
    days = np.floor_divide(utc_ms, DAY_MS)
    # year from the UTC day, as the Rust does
    dt = pd.to_datetime(days*DAY_MS, unit='ms', utc=True)
    years = dt.year.values.astype(np.int64)
    uy = np.unique(years)
    start = {}; end = {}
    for y in uy:
        start[y] = nth_sunday(int(y), 3, 2)*DAY_MS + 7*HOUR_MS
        end[y]   = nth_sunday(int(y), 11, 1)*DAY_MS + 6*HOUR_MS
    s = np.array([start[y] for y in years], dtype=np.int64)
    e = np.array([end[y]   for y in years], dtype=np.int64)
    is_dst = (utc_ms >= s) & (utc_ms < e)
    return np.where(is_dst, -4*HOUR_MS, -5*HOUR_MS).astype(np.int64)

def load(clock='ny'):
    df = pd.read_parquet(PATH, columns=['time','open','high','low','close'])
    utc_ms = df['time'].values.astype('datetime64[ms]').astype(np.int64)
    if clock == 'ny':
        local = utc_ms + ny_offset_ms(utc_ms)
    elif clock == 'utc':
        local = utc_ms.copy()
    else:
        raise ValueError(clock)
    df = df.drop(columns=['time'])
    df['utc_ms'] = utc_ms
    df['day'] = np.floor_divide(local, DAY_MS)
    df['hour'] = np.floor_divide(local % DAY_MS, HOUR_MS)
    df['wd'] = (df['day'] + 4) % 7
    return df
