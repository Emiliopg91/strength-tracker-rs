
run:
	@RUSTC_WRAPPER=sccache mold --run pnpm tauri dev -- -- -v

build:
	@mold --run pnpm tauri build

lint:
	@echo "Linting frontend"
	@pnpm lint
	@echo "Linting macros project"
	@cd src-tauri-macros && cargo clippy
	@echo "Linting backend"
	@cd src-tauri && cargo clippy

clean:
	@rm -Rf node_modules dist && cd src-tauri && cargo clean

release:
	@python resources/scripts/release.py 

setup-toolchain:
	@paru -S $(shell bash -c 'source resources/PKGBUILD && printf "%s " "$${makedepends[@]}" "$${depends[@]}"')

update:
	@python resources/scripts/update-dependencies.py

publish:
	@RUSTC_WRAPPER=sccache python resources/scripts/publish.py

test:
	@pnpm test
	@cd src-tauri && cargo test --lib --bins --tests -- --test-threads=1

dump:
	@./src-tauri/target/debug/strength-coach-for-garmin --dump

decode:
	@test -n "$(FILE)" || { echo "Usage: make decode FILE=<path.fit> [FILE=\"a.fit b.fit\"]"; exit 1; }
	@./src-tauri/target/debug/strength-coach-for-garmin --decode $(FILE)