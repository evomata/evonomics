# evonomics

![3](https://evomata.github.io/evonomics/3.png)
![2](https://evomata.github.io/evonomics/2.png)
![1](https://evomata.github.io/evonomics/1.png)
![0](https://evomata.github.io/evonomics/0.png)

## Profiling

Do this so that you can profile with perf:

```bash
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

Install `flamegraph`:

```bash
cargo install flamegraph
```

Profile it:

```bash
cargo flamegraph
```
