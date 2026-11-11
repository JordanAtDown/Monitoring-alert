#!/usr/bin/env python3
"""Creates a database in the 1.x format with 14 months of synthetic history, for local tests.

Several components of each type (three disks, a motherboard…), a seasonal swing, and a slow
drift on the processor and one disk, so that every part of the dashboard has something to show.
The processor also draws power and has a fan that speeds up with its temperature, so that the
cooling card has a story to tell: the processor keeps more and more heat for the same watts.

Usage: demo_seed.py <database>
"""
import datetime, math, random, sqlite3, sys

random.seed(7)
# Fans and power come from their own generator: adding them leaves the temperatures unchanged.
extras_rng = random.Random(11)
SENSORS = [  # hardware, sensor, base temperature, drift per day, noise
    ("AMD Ryzen 7 5700X", "Core (Tctl/Tdie)", 36, 0.015, 1.2),
    ("AMD Ryzen 7 5700X", "CCD1 (Tdie)", 37, 0.015, 1.2),
    ("NVIDIA GeForce RTX 5060", "GPU Core", 31, 0.0, 1.0),
    ("NVIDIA GeForce RTX 5060", "GPU Memory Junction", 37, 0.0, 1.0),
    ("Gigabyte B550I AORUS PRO AX", "System", 28, 0.0, 0.8),
    ("Gigabyte B550I AORUS PRO AX", "Chipset", 40, 0.0, 0.8),
    ("Gigabyte B550I AORUS PRO AX", "VRM MOS", 37, 0.005, 0.8),
    ("Samsung SSD 970 EVO Plus 250GB", "Composite Temperature", 50, 0.01, 1.0),
    ("Samsung SSD 970 EVO Plus 250GB", "Temperature #2", 58, 0.012, 1.0),
    ("CT500P310SSD8", "Composite Temperature", 40, 0.0, 1.0),
    ("Samsung SSD 860 QVO 1TB ", "Temperature", 30, 0.0, 0.7),
]

db = sqlite3.connect(sys.argv[1])
db.executescript("""
CREATE TABLE snapshots (id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, cpu_load REAL, gpu_load REAL, load_cat TEXT NOT NULL);
CREATE TABLE readings (id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER NOT NULL REFERENCES snapshots(id), hardware TEXT NOT NULL, sensor TEXT NOT NULL, value REAL NOT NULL);
CREATE INDEX idx_snapshots_ts ON snapshots(ts);
CREATE INDEX idx_readings_snapshot ON readings(snapshot_id);
CREATE INDEX idx_readings_hw_sensor ON readings(hardware, sensor);
CREATE TABLE extra_readings (id INTEGER PRIMARY KEY AUTOINCREMENT, snapshot_id INTEGER NOT NULL REFERENCES snapshots(id), hardware TEXT NOT NULL, sensor TEXT NOT NULL, kind TEXT NOT NULL, value REAL NOT NULL);
CREATE INDEX idx_extra_readings_snapshot ON extra_readings(snapshot_id);
""")
now = datetime.datetime.now().replace(minute=0, second=0, microsecond=0)
t = now - datetime.timedelta(days=420)
while t < now:
    day = (now - t).days
    if random.random() < 0.8:  # the PC is not always on
        season = 3 * math.sin(2 * math.pi * (t.timetuple().tm_yday - 100) / 365)
        heavy = random.random() < 0.15
        cat = "heavy" if heavy else "idle"
        cur = db.execute("INSERT INTO snapshots (ts, cpu_load, gpu_load, load_cat) VALUES (?,?,?,?)",
                         (t.strftime("%Y-%m-%dT%H:%M:%S"), random.uniform(60, 95) if heavy else random.uniform(1, 10), 1.0, cat))
        rows = [(cur.lastrowid, h, s, base + season + drift * (420 - day) + (35 if heavy else 0) + random.gauss(0, noise)) for h, s, base, drift, noise in SENSORS]
        db.executemany("INSERT INTO readings (snapshot_id, hardware, sensor, value) VALUES (?,?,?,?)", rows)
        cpu_temp = next(v for _, h, s, v in rows if s == "Core (Tctl/Tdie)")
        g = extras_rng.gauss
        extras = [
            ("AMD Ryzen 7 5700X", "Package", "power", (95 if heavy else 35) + g(0, 2)),
            # A fan driven by the processor temperature: it speeds up when the processor gets hotter.
            ("Gigabyte B550I AORUS PRO AX", "CPU Fan", "fan", max(600.0, 700 + 30 * (cpu_temp - 30) + g(0, 25))),
            ("Gigabyte B550I AORUS PRO AX", "System Fan #1", "fan", 650 + g(0, 15)),
            ("NVIDIA GeForce RTX 5060", "GPU Package", "power", (120 if heavy else 8) + g(0, 1)),
            ("NVIDIA GeForce RTX 5060", "GPU Fan", "fan", 1500 + g(0, 40) if heavy else 0.0),
        ]
        db.executemany("INSERT INTO extra_readings (snapshot_id, hardware, sensor, kind, value) VALUES (?,?,?,?,?)",
                       [(cur.lastrowid, h, s, k, v) for h, s, k, v in extras])
    t += datetime.timedelta(minutes=30)
db.commit()
print("snapshots:", db.execute("select count(*) from snapshots").fetchone()[0])
