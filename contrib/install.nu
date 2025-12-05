### Install the-editor to /usr/bin

let bin_path = "../target/release/the-editor"
let dest     = "/usr/bin/the-editor"

sudo cp $bin_path $dest
sudo chmod +x $dest

# if $dest {
#     echo "Binary installed"
# } else {
#     echo "Failed to install."
# }
