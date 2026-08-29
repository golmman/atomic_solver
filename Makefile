.PHONY: quick_export macos_cleanup nn_corpus test test-full test-lite

test:       ## fast gate: unit + fast integration tests (~1 min of test time)
	CARGO_PROFILE_RELEASE_LTO=thin cargo test --release

test-full:  ## everything, incl. 60 s regression/stress suites (~25 min)
	cargo test --release -- --include-ignored

test-lite:  ## debug build, quick logic check
	cargo test

quick_export:
	cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10

quick_export2:
	cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10

macos_cleanup:
	find . -name ".DS_Store" -print -delete

nn_corpus:
	rm -rf data/corpus/
	cargo run --release --example corpus_gen -- solve --suite quick --timeout 20 --dump-dir data/corpus/trees
	cargo run --release --example corpus_gen -- load  --dump-dir data/corpus/trees --output data/corpus/train.ndjson
