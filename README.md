## Usage

Currently, the program expects the env variables `SSH_KEY_PATH`, `SSH_USERNAME`, and `SSH_HOST` to be set.
Instead of `SSH_KEY_PATH` a combination of `SSH_PASSWORD` and `SSH_MFA` can be used instead.

To run the program specify it before cargo run, for instance as follows:

```
SSH_USERNAME=you SSH_KEY_PATH=~/.ssh/id_your_key SSH_HOST=hpc.example.com cargo run --release
```

or 

```
SSH_USERNAME=you SSH_PASSWORD="your-pw" SSH_MFA=31234 SSH_HOST=hpc.example.com cargo run --release
```