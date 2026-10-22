"""Validate a rewritten ReRu AOF with redis-check-aof and load it in Redis.

Uses dedicated servers and temporary files; never touches an existing database.
Windows: python scripts/test_aof_rewrite_interop.py --wsl Ubuntu --reru target/release/reru.exe
Linux: python3 scripts/test_aof_rewrite_interop.py --reru target/release/reru
"""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
import time

from test_rdb_interop import port, seed, start, verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wsl")
    parser.add_argument("--reru", required=True)
    args = parser.parse_args()
    prefix = ["wsl", "-d", args.wsl, "--"] if args.wsl else []
    root = pathlib.Path(__file__).resolve().parents[1]
    (root / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="aof-interop-", dir=root / "target") as name:
        directory = pathlib.Path(name)
        redis_dir = subprocess.check_output(prefix + ["wslpath", "-a", directory.as_posix()], text=True).strip() if args.wsl else name
        processes = []
        try:
            with (directory / "servers.log").open("wb") as log:
                number = port()
                process, reru = start([str(pathlib.Path(args.reru).resolve()), "--port", str(number),
                    "--appendonly", str(directory / "reru.aof")], number, log)
                processes.append((process, reru, False))
                seed(reru)
                reru.call("XADD", "stream", "5-0", "f", "v")
                reru.call("XGROUP", "CREATE", "stream", "workers", "0")
                reru.call("XREADGROUP", "GROUP", "workers", "alice", "STREAMS", "stream", ">")
                reru.call("XDEL", "stream", "5-0")
                reru.call("XGROUP", "CREATE", "empty-stream", "temporary", "0", "MKSTREAM")
                reru.call("XGROUP", "DESTROY", "empty-stream", "temporary")
                reru.call("XADD", "trimmed-stream", "9-0", "f", "v")
                reru.call("XGROUP", "CREATE", "trimmed-stream", "g", "0")
                reru.call("XREADGROUP", "GROUP", "g", "alice", "STREAMS", "trimmed-stream", ">")
                reru.call("XTRIM", "trimmed-stream", "MAXLEN", "0")
                reru.call("XSETID", "trimmed-stream", "0-0")
                reru.call("XADD", "trimmed-stream", "0-1", "f", "new")
                for _ in range(30):
                    reru.call("SET", "history", b"x" * 16384)
                reru.call("DEL", "history")
                assert reru.call("BGREWRITEAOF") == b"Background append only file rewriting started"
                for _ in range(20):
                    reru.call("INCR", "counter")
                deadline = time.monotonic() + 10
                while b"aof_rewrite_in_progress:1\r\n" in reru.call("INFO", "persistence"):
                    assert time.monotonic() < deadline, "rewrite timeout"
                    time.sleep(0.02)
                assert b"aof_last_bgrewrite_status:ok\r\n" in reru.call("INFO", "persistence")
                shutil.copyfile(directory / "reru.aof", directory / "redis.aof")
                checked = subprocess.run(prefix + ["redis-check-aof", redis_dir + "/redis.aof"], capture_output=True, text=True)
                assert checked.returncode == 0, checked.stdout + checked.stderr
                number = port()
                process, redis = start(prefix + ["redis-server", "--bind", "127.0.0.1", "--port", str(number),
                    "--dir", redis_dir, "--appendonly", "yes", "--appendfilename", "redis.aof", "--save", ""], number, log)
                processes.append((process, redis, True))
                verify(reru, redis)
                assert redis.call("GET", "counter") == b"20"
                assert redis.call("GET", "history") is None
                assert redis.call("XLEN", "stream") == 0
                assert redis.call("XPENDING", "stream", "workers") == reru.call("XPENDING", "stream", "workers")
                assert redis.call("XREADGROUP", "GROUP", "workers", "alice", "STREAMS", "stream", "0") == reru.call("XREADGROUP", "GROUP", "workers", "alice", "STREAMS", "stream", "0")
                assert redis.call("XADD", "empty-stream", "0-1", "f", "v") == reru.call("XADD", "empty-stream", "0-1", "f", "v") == b"0-1"
                assert redis.call("XADD", "stream", "5-*", "f", "v") == reru.call("XADD", "stream", "5-*", "f", "v") == b"5-1"
                for command in [("XPENDING", "trimmed-stream", "g"), ("XRANGE", "trimmed-stream", "-", "+")]:
                    assert redis.call(*command) == reru.call(*command)
                for key in ["stream", "trimmed-stream", "empty-stream"]:
                    ours = reru.call("XINFO", "STREAM", key)
                    reference = redis.call("XINFO", "STREAM", key)
                    ours = dict(zip(ours[::2], ours[1::2]))
                    reference = dict(zip(reference[::2], reference[1::2]))
                    for field in [b"entries-added", b"max-deleted-entry-id", b"last-generated-id", b"length", b"groups"]:
                        assert ours[field] == reference[field], (key, field, ours, reference)
                print("PASS: rewritten ReRu AOF -> redis-check-aof -> Redis; collections, TTL, stream IDs, groups, deleted pending entries and subsequent writes")
        finally:
            for process, client, is_redis in reversed(processes):
                if is_redis and process.poll() is None:
                    try:
                        client.call("SHUTDOWN", "NOSAVE")
                    except (OSError, EOFError):
                        pass
                elif process.poll() is None:
                    process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                client.close()


if __name__ == "__main__":
    main()
