.PHONY: macos_cleanup
.PHONY: test test-full test-lite
.PHONY: stress quick_check quick_check2 quick_check3
.PHONY: proofdb_init proofdb_cycle proofdb_shard_count

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

quick_check:
	cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10

quick_check2:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10

quick_check3:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --tt-dump-path proof_tree.bin.tt
	cargo run --release --example reconstruct_pt -- --snapshot proof_tree.bin.tt --out proof_tree.bin

stress:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21" --timeout 60

# Proofdb

proofdb_init:
	rm -rf data/proofdb
	cargo build --release --examples
	mkdir -p data/proofdb/shards
	printf '{"entries":[]}' > data/proofdb/shards/manifest.json
	$(BIN)/proofdb_merge
	$(BIN)/proofdb_harvest --policy and-close --budget-evals 200000 --max-jobs 2
	
	echo "Next: creating bootstrap batch"
	sleep 10
	$(BIN)/proofdb_harvest --policy descend
	$(BIN)/proofdb_merge    # fold the batch's shards into the DB (the harvest never merges)
	$(BIN)/proofdb_flip     # soundness sanity over the grown DB (expect: flips 0 verified 0)

proofdb_cycle:
	cargo build --release --examples
	$(BIN)/proofdb_harvest --policy and-close --budget-evals 4000000 \
		--and-close-max-budget 100000000 --tt-mb 1024 \
		--max-total-evals 10000000000
	$(BIN)/proofdb_merge    # fold the batch's shards into the DB (the harvest never merges)
	$(BIN)/proofdb_flip     # soundness sanity over the grown DB (expect: flips 0 verified 0)

proofdb_shard_count:
	ls data/proofdb/shards/ | wc -l
