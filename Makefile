.PHONY: macos_cleanup
.PHONY: test test-full test-lite
.PHONY: stress quick_loss quick_draw quick_win quick_win2
.PHONY: proofdb_init proofdb_cycle_run proofdb_cycle_stop proofdb_stats proofdb_sync

BIN := target/release/examples

# General Utilities

macos_cleanup:
	find . -name ".DS_Store" -print -delete

# Tests

test:       ## fast gate: unit + fast integration tests (~1 min of test time)
	CARGO_PROFILE_RELEASE_LTO=thin cargo test --release

test-full:  ## everything, incl. 60 s regression/stress suites (~25 min)
	cargo test --release -- --include-ignored

test-lite:  ## debug build, quick logic check
	cargo test

# Cli Shortcuts

quick_loss:
	cargo run --release -- --fen "rnb1kbnr/pppp1ppp/4p3/8/7q/NP6/P1PPPPPP/R1BQKBNR w KQkq - 1 3" --timeout 10

quick_draw:
	cargo run --release -- --fen "7k/5p1P/5P2/2p5/1NPB4/8/8/K7 w - - 0 1" --timeout 10

quick_win:
	cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10

quick_win2:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10

stress:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21" --timeout 60

# Proofdb

proofdb_init:
	rm -rf data/proofdb
	cargo build --release --examples
	mkdir -p data/proofdb/shards
	printf '{"entries":[]}' > data/proofdb/shards/manifest.json
	$(BIN)/proofdb_merge

proofdb_cycle_run:
	-rm STOP
	cargo build --release --examples
#	$(BIN)/proofdb_harvest --policy frontier-walk --budget-evals 100000000 --moves "g1f3 d7d6 f3g5 f7f6"
	$(BIN)/proofdb_harvest --policy frontier-walk --budget-evals 100000000 --moves "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6"
	$(BIN)/proofdb_merge
	$(BIN)/proofdb_flip
	-rm STOP

proofdb_cycle_stop:
	touch STOP

proofdb_stats:
	@printf "number of shards: "
	@ls data/proofdb/shards/ | wc -l
	@printf "size of proofdb.db: "
	@wc -c < data/proofdb/proofdb.db

proofdb_sync:
	examples/proofdb/sync.sh
