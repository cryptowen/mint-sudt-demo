schema:
	echo '#![allow(dead_code)]' > tmp_file
	moleculec --language rust --schema-file schema/eth_bridge.mol >> tmp_file
	mv tmp_file contracts/eth-bridge-typescript/src/types.rs
	cd contracts/eth-bridge-typescript && cargo fmt
	cp contracts/eth-bridge-typescript/src/types.rs tests/src/types.rs

fmt:
	cd contracts/wckb-typescript && cargo fmt

.PHONY: schema