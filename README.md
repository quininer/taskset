# 0-unsafe `FuturesUnordered` alternate

Simple `FuturesUnordered` alternative, but 0-unsafe.
Performance is even better than `FuturesUnordered` in some benchmarks.

```
taskset                 time:   [3.7362 ms 3.8423 ms 3.9553 ms]
                        change: [-4.6928% -1.8529% +1.1773%] (p = 0.23 > 0.05)
                        No change in performance detected.
Found 1 outliers among 30 measurements (3.33%)
  1 (3.33%) high mild

futures_unordered       time:   [4.0081 ms 4.1792 ms 4.3758 ms]
                        change: [-2.2008% +3.5122% +9.2941%] (p = 0.25 > 0.05)
                        No change in performance detected.
Found 3 outliers among 30 measurements (10.00%)
  3 (10.00%) high mild
```

And it has [a lock-free branch](https://github.com/quininer/taskset/tree/lockfree) based on `crossbeam-queue`.
