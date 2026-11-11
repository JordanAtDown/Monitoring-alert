#!/usr/bin/env python3
"""Fake LibreHardwareMonitor for local tests: replays the sensors of the latest snapshot of a database.

Usage: demo_lhm.py <database> <port>
"""
import http.server, json, random, sqlite3, sys

DEFAULT = {  # used when the database has no snapshot yet
    "AMD Ryzen 7 5700X": [("Core (Tctl/Tdie)", 45.0), ("CCD1 (Tdie)", 44.0)],
    "NVIDIA GeForce RTX 5060": [("GPU Core", 32.0), ("GPU Memory Junction", 36.0)],
}


def latest_sensors(path):
    db = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    row = db.execute("select id from snapshots order by ts desc limit 1").fetchone()
    if not row:
        return DEFAULT
    hardware = {}
    for h, s, v in db.execute("select hardware, sensor, value from readings where snapshot_id=? order by hardware, sensor", row):
        hardware.setdefault(h, []).append((s, v))
    return hardware or DEFAULT


def fr(value, unit):
    return f"{value:.1f}".replace(".", ",") + f" {unit}"


def tree(hardware):
    nodes = []
    for name, sensors in hardware.items():
        gpu = any(s.startswith("GPU ") for s, _ in sensors)
        cpu = any("Tdie" in s or "Tctl" in s for s, _ in sensors)
        temps = [{"Text": s, "Value": fr(v + random.uniform(-0.4, 0.4), "°C"), "ImageURL": "", "Type": "Temperature", "Children": []} for s, v in sensors]
        children = [{"Text": "Temperatures", "Value": "", "ImageURL": "", "Type": "", "Children": temps}]
        for kind, load_name, active in (("cpu", "CPU Total", cpu), ("gpu", "GPU Core", gpu)):
            if active:
                load = {"Text": load_name, "Value": fr(random.uniform(0, 6), "%"), "ImageURL": "", "Type": "Load", "Children": []}
                children.append({"Text": "Load", "Value": "", "ImageURL": "", "Type": "", "Children": [load]})
        icon = "images_icon/nvidia.png" if gpu else "images_icon/cpu.png" if cpu else ""
        nodes.append({"Text": name, "Value": "", "ImageURL": icon, "Type": "", "Children": children})
    pc = {"Text": "PC", "Value": "", "ImageURL": "", "Type": "", "Children": nodes}
    return {"Text": "Sensor", "Value": "", "ImageURL": "", "Type": "", "Children": [pc]}


if __name__ == "__main__":
    sensors = latest_sensors(sys.argv[1])

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            body = json.dumps(tree(sensors)).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[2])), Handler).serve_forever()
