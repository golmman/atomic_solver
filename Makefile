.PHONY: quick_export macos_cleanup

quick_export:
	cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10

macos_cleanup:
	find . -name ".DS_Store" -print -delete
