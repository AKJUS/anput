list:
  just --list

format:
  cargo fmt --all

build:
  cargo build --all --all-features

test:
  cargo test --all --all-features -- --nocapture

miri:
  cargo +nightly miri test --manifest-path ./crates/jobs/Cargo.toml -- --nocapture
  cargo +nightly miri test --manifest-path ./crates/generator/Cargo.toml -- --nocapture
  cargo +nightly miri test --manifest-path ./crates/_/Cargo.toml -- --nocapture
  cargo +nightly miri test --manifest-path ./crates/spatial/Cargo.toml -- --nocapture
  cargo +nightly miri test --manifest-path ./crates/physics/Cargo.toml -- --nocapture

clippy:
  cargo clippy --all --all-features
  cargo clippy --tests --all --all-features

checks:
  just format
  just build
  just clippy
  just test
  just miri

demo:
  cargo run --release --manifest-path ./demo/Cargo.toml

clear-benchmarks:
  rm -rf ./vtune-reports
  mkdir ./vtune-reports

run-benchmarks:
  cargo build --profile profiling --bins --manifest-path ./bench/Cargo.toml
  vtune -collect hotspots -finalization-mode=full -result-dir=./vtune-reports/spawn-r@@@{at} -- ./target/profiling/spawn.exe
  vtune -collect hotspots -finalization-mode=full -result-dir=./vtune-reports/query-r@@@{at} -- ./target/profiling/query.exe
  vtune -collect hotspots -finalization-mode=full -result-dir=./vtune-reports/lookup-r@@@{at} -- ./target/profiling/lookup.exe
  vtune -collect hotspots -finalization-mode=full -result-dir=./vtune-reports/lookup-access-r@@@{at} -- ./target/profiling/lookup_access.exe

run-fresh-benchmarks:
  just clear-benchmarks
  just run-benchmarks

clean:
  find . -name target -type d -exec rm -r {} +
  just remove-lockfiles

remove-lockfiles:
  find . -name Cargo.lock -type f -exec rm {} +

list-outdated:
  cargo outdated -R -w

update:
  cargo update --manifest-path ./crates/jobs/Cargo.toml --aggressive
  cargo update --manifest-path ./crates/generator/Cargo.toml --aggressive
  cargo update --manifest-path ./crates/_/Cargo.toml --aggressive
  cargo update --manifest-path ./crates/spatial/Cargo.toml --aggressive
  cargo update --manifest-path ./crates/physics/Cargo.toml --aggressive

publish:
  cargo publish --no-verify --manifest-path ./crates/jobs/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./crates/generator/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./crates/_/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./crates/spatial/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./crates/physics/Cargo.toml