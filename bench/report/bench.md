# ksbh bench report

- generated: 2026-06-06T23:40:09Z
- git SHA:   a4d9bd84e97cacfb9fa313e7f2d2efda519a6e72
- result files: 18 (9 scenarios)
- machine:   10 CPUs, Apple M1 Pro (25.5.0) @ MacBook-Pro-de-Sarah.local

## `robust_h2_streams`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 164979.185 | 131489.823 | -33489.362 | -20.30% |
| p50 (ms) | 0.009 | 0.009 | 0 | -2.61% |
| p99 (ms) | 0.129 | 0.142 | +0.013 | 9.86% |
| max (ms) | 127.243 | 128.077 | +0.834 | 0.66% |
| peak RSS (KiB) | 53504 | 7948 | -45556 | -85.15% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00003% | +0 | 25.47% |

## `robust_malformed`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 178627.344 | 177333.330 | -1294.014 | -0.72% |
| p50 (ms) | 0.007 | 0.007 | +0 | 0.99% |
| p99 (ms) | 0.091 | 0.093 | +0.002 | 2.03% |
| max (ms) | 122.149 | 122.085 | -0.064 | -0.05% |
| peak RSS (KiB) | 53328 | 7448 | -45880 | -86.03% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | +0 | 0.73% |

## `robust_slow_loris`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 179553.271 | 177601.698 | -1951.573 | -1.09% |
| p50 (ms) | 0.007 | 0.007 | 0 | -0.78% |
| p99 (ms) | 0.087 | 0.085 | -0.002 | -2.30% |
| max (ms) | 128.779 | 169.496 | +40.717 | 31.62% |
| peak RSS (KiB) | 53664 | 7448 | -46216 | -86.12% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | +0 | 1.10% |

## `robust_upstream_kill`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 91894.583 | 178308.139 | +86413.556 | 94.04% |
| p50 (ms) | 0.007 | 0.007 | +0 | 1.22% |
| p99 (ms) | 0.089 | 0.087 | -0.002 | -2.22% |
| max (ms) | 10001.391 | 252.999 | -9748.392 | -97.47% |
| peak RSS (KiB) | 53696 | 7448 | -46248 | -86.13% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 1 | 0 | -1 | -100.00% |
| error rate | 0.00007% | 0.00002% | 0 | -74.23% |

## `speed_const_50krps`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 49975.337 | 50000.168 | +24.831 | 0.05% |
| p50 (ms) | 0.007 | 0.007 | +0 | 3.38% |
| p99 (ms) | 0.063 | 0.067 | +0.004 | 6.89% |
| max (ms) | 191.086 | 194.603 | +3.517 | 1.84% |
| peak RSS (KiB) | 51888 | 7452 | -44436 | -85.64% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00007% | 0.00007% | 0 | -0.05% |

## `speed_large_c50`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 168257.995 | 148806.373 | -19451.622 | -11.56% |
| p50 (ms) | 0.007 | 0.007 | +0 | 2.04% |
| p99 (ms) | 0.093 | 0.102 | +0.009 | 9.73% |
| max (ms) | 277.066 | 275.537 | -1.528 | -0.55% |
| peak RSS (KiB) | 53504 | 7452 | -46052 | -86.07% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | +0 | 13.07% |

## `speed_small_c1`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 169869.071 | 167484.727 | -2384.344 | -1.40% |
| p50 (ms) | 0.007 | 0.007 | +0 | 1.49% |
| p99 (ms) | 0.095 | 0.096 | +0.001 | 1.41% |
| max (ms) | 120.818 | 121.184 | +0.367 | 0.30% |
| peak RSS (KiB) | 53536 | 7448 | -46088 | -86.09% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | +0 | 1.42% |

## `speed_small_c100`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 176806.682 | 177012.871 | +206.189 | 0.12% |
| p50 (ms) | 0.007 | 0.007 | +0 | 1.31% |
| p99 (ms) | 0.092 | 0.092 | 0 | -0.30% |
| max (ms) | 239.049 | 256.366 | +17.318 | 7.24% |
| peak RSS (KiB) | 53568 | 7452 | -46116 | -86.09% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | 0 | -0.12% |

## `speed_tls_h2`

| metric | ksbh | nginx | delta | delta % |
|---|---:|---:|---:|---:|
| RPS | 151607.255 | 144589.693 | -7017.563 | -4.63% |
| p50 (ms) | 0.010 | 0.009 | 0 | -1.75% |
| p99 (ms) | 0.138 | 0.138 | +0 | 0.34% |
| max (ms) | 386.814 | 191.915 | -194.898 | -50.39% |
| peak RSS (KiB) | 53904 | 7948 | -45956 | -85.26% |
| errors · 5xx | 0 | 0 | — | — |
| errors · 4xx | 0 | 0 | — | — |
| errors · connect_failed | 1 | 1 | 0 | 0.00% |
| errors · read_timeout | 0 | 0 | — | — |
| error rate | 0.00002% | 0.00002% | +0 | 4.85% |

## Failures

_none_
