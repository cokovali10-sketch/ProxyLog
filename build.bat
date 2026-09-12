@echo off
echo Building ProxyLog...
cargo build --release
echo Done: target\release\proxylog.exe
copy target\release\proxylog.exe . 2>nul
