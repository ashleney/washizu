git clone https://github.com/Equim-chan/Mortal.git
find Mortal -type f -name '*.rs' -exec sed -i 's/pub(super)/pub/g' {} +
find Mortal -type f -name '*.rs' -exec sed -i 's/^mod /pub mod /' {} +
