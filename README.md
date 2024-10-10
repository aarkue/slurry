## Usage

Currently, the program expects the env variables `SSH_KEY_PATH`, `USERNAME`, and `SSH_HOST` to be set.

To run the program specify it before cargo run, for instance as follows:

```
SSH_USERNAME=you SSH_KEY_PATH=~/.ssh/id_your_key SSH_HOST=hpc.example.com cargo run --release
```