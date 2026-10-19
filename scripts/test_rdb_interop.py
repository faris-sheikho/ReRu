"""Exchange full RDB files with an installed Redis server (no Python packages needed).

Windows: python scripts/test_rdb_interop.py --wsl Ubuntu --reru target/debug/reru.exe
Linux:   python3 scripts/test_rdb_interop.py --reru target/debug/reru
Uses temporary directories and dedicated ephemeral ports; never an existing database.
"""
import argparse
import pathlib
import shutil
import socket
import subprocess
import tempfile
import time


class Client:
    def __init__(self, port):
        self.socket = socket.create_connection(("127.0.0.1", port), timeout=3)
        self.file = self.socket.makefile("rb")

    def close(self):
        self.file.close()
        self.socket.close()

    def call(self, *args):
        args = [arg if isinstance(arg, bytes) else str(arg).encode() for arg in args]
        wire = b"*%d\r\n" % len(args)
        for arg in args:
            wire += b"$%d\r\n" % len(arg) + arg + b"\r\n"
        self.socket.sendall(wire)
        return self.read()

    def read(self):
        line = self.file.readline()
        if not line.endswith(b"\r\n"):
            raise EOFError("connection closed")
        kind, value = line[:1], line[1:-2]
        if kind == b"+":
            return value
        if kind == b"-":
            raise AssertionError(value.decode(errors="replace"))
        if kind == b":":
            return int(value)
        if kind == b"$":
            length = int(value)
            if length == -1:
                return None
            data = self.file.read(length)
            assert self.file.read(2) == b"\r\n"
            return data
        if kind == b"*":
            return [self.read() for _ in range(int(value))]
        raise AssertionError(f"unknown response: {line!r}")


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def start(command, number, log):
    process = subprocess.Popen(command, stdout=log, stderr=log)
    try:
        until = time.monotonic() + 10
        while process.poll() is None and time.monotonic() < until:
            try:
                return process, Client(number)
            except OSError:
                time.sleep(0.05)
        raise AssertionError(f"server failed to start: {command}")
    except BaseException:
        process.kill()
        process.wait()
        raise


def seed(client):
    commands = [
        ("SET", "binary", b"\x00\xff\r\n"),
        ("SET", "integer", "-2000000000"),
        ("SET", "compressed", b"abc" * 12000),
        ("RPUSH", "list", "127", "-4096", "8388607", b"\0"),
        ("HSET", "hash", "field", "value", "integer", "300"),
        ("SADD", "set", "alpha", "beta"),
        ("SADD", "intset", "-32769", "42", "9223372036854775807"),
        ("ZADD", "sorted", "1.25", "a", "-inf", "b"),
        ("GEOADD", "geo", "13.361389", "38.115556", "Palermo"),
        ("SET", "ttl", "expires", "PX", "60000"),
    ]
    for command in commands:
        client.call(*command)


def verify(left, right):
    for query in [
        ("GET", "binary"), ("GET", "integer"), ("GET", "compressed"),
        ("LRANGE", "list", "0", "-1"), ("HMGET", "hash", "field", "integer"),
        ("SMISMEMBER", "set", "alpha", "beta", "absent"),
        ("SMISMEMBER", "intset", "-32769", "42", "9223372036854775807"),
        ("ZRANGE", "sorted", "0", "-1", "WITHSCORES"),
        ("GEOPOS", "geo", "Palermo"), ("GET", "ttl"),
    ]:
        assert left.call(*query) == right.call(*query), query
    assert 0 < left.call("PTTL", "ttl") <= 60000
    # Compare stored deadlines: native Windows and WSL wall clocks may differ.
    assert abs(left.call("PEXPIRETIME", "ttl") - right.call("PEXPIRETIME", "ttl")) < 10


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wsl", help="Windows WSL distribution containing Redis")
    parser.add_argument("--reru", required=True)
    args = parser.parse_args()
    prefix = ["wsl", "-d", args.wsl, "--"] if args.wsl else []
    root = pathlib.Path(__file__).resolve().parents[1]
    (root / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rdb-interop-", dir=root / "target") as name:
        directory = pathlib.Path(name)
        redis_dir = subprocess.check_output(prefix + ["wslpath", "-a", directory.as_posix()], text=True).strip() if args.wsl else name
        processes = []
        clients = []
        try:
            with (directory / "servers.log").open("wb") as log:
                def reference(filename):
                    number = port()
                    process, client = start(prefix + ["redis-server", "--bind", "127.0.0.1", "--port", str(number),
                        "--dir", redis_dir, "--dbfilename", filename, "--save", "", "--appendonly", "no"], number, log)
                    processes.append((process, client, True)); clients.append(client)
                    return client

                original = reference("reference.rdb")
                seed(original)
                assert original.call("SAVE") == b"OK"
                shutil.copyfile(directory / "reference.rdb", directory / "reru.rdb")
                number = port()
                process, rebuilt = start([str(pathlib.Path(args.reru).resolve()), "--port", str(number),
                    "--dir", name, "--dbfilename", "reru.rdb"], number, log)
                processes.append((process, rebuilt, False)); clients.append(rebuilt)
                verify(rebuilt, original)
                assert rebuilt.call("SET", "written-by-reru", "yes") == b"OK"
                assert rebuilt.call("SAVE") == b"OK"
                checked = subprocess.run(prefix + ["redis-check-rdb", redis_dir + "/reru.rdb"], capture_output=True, text=True)
                assert checked.returncode == 0, checked.stdout + checked.stderr
                restored = reference("reru.rdb")
                verify(restored, rebuilt)
                assert restored.call("GET", "written-by-reru") == b"yes"
                print("PASS: Redis -> RDB -> ReRu -> RDB -> Redis; CRC, collections, geo, binary data, compression and TTL")
        finally:
            for process, client, redis in reversed(processes):
                if redis and process.poll() is None:
                    try:
                        client.call("SHUTDOWN", "NOSAVE")
                    except (OSError, EOFError):
                        pass
                elif process.poll() is None:
                    process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait()
            for client in clients:
                client.close()


if __name__ == "__main__":
    main()
