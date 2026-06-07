.PHONY: install dev build preview check check-lint check-lint-fix fmt test rust-test rust-check tauri-dev tauri-dev-smoke tauri-build clean

SHELL := powershell.exe
.SHELLFLAGS := -NoProfile -ExecutionPolicy Bypass -Command

BUN := bun
CARGO := cargo

install:
	$(BUN) install

dev:
	$(BUN) run dev

build:
	$(BUN) run build

preview:
	$(BUN) run preview

check:
	$(BUN) run check
	Set-Location src-tauri; $(CARGO) check

check-lint:
	$$bash = @('D:\Scoop\apps\git\current\bin\bash.exe', 'C:\Program Files\Git\bin\bash.exe', 'C:\Program Files\Git\usr\bin\bash.exe') | Where-Object { Test-Path $$_ } | Select-Object -First 1; if (-not $$bash) { $$bash = 'bash' }; & $$bash scripts/check_lint.sh

check-lint-fix:
	$$bash = @('D:\Scoop\apps\git\current\bin\bash.exe', 'C:\Program Files\Git\bin\bash.exe', 'C:\Program Files\Git\usr\bin\bash.exe') | Where-Object { Test-Path $$_ } | Select-Object -First 1; if (-not $$bash) { $$bash = 'bash' }; & $$bash scripts/check_lint.sh --fix

fmt:
	Set-Location src-tauri; $(CARGO) fmt

test: rust-test

rust-test:
	Set-Location src-tauri; $(CARGO) test

rust-check:
	Set-Location src-tauri; $(CARGO) check

tauri-dev:
	$(BUN) run tauri dev

tauri-dev-smoke:
	New-Item -ItemType Directory -Force tmp | Out-Null; $(BUN) run tauri dev 1> tmp/tauri-dev-smoke.out.log 2> tmp/tauri-dev-smoke.err.log

tauri-build:
	$(BUN) run tauri build

clean:
	$(BUN) pm cache rm
	Set-Location src-tauri; $(CARGO) clean
