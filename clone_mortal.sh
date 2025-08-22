git clone https://github.com/Equim-chan/Mortal.git
cargo run --release --manifest-path rust_public/Cargo.toml Mortal/libriichi
find Mortal -type f -name '*.rs' -exec sed -i 's/SHANTEN_THRES: i8 = 3/SHANTEN_THRES: i8 = 5/' {} +
