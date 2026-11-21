# BUDGET — orçamento de memória (RSS de pico)

Piso de eficiência de runtime da casa (`eficiencia-runtime.md`): cada operação declara um teto de RSS de pico, medido e imposto por teste. Teto da casa para CLI: 256 MiB. Aqui o medido é uma ordem de grandeza abaixo.

| Operação | RSS pico alvo (release) | Medido release (antes -> depois) | Medido debug (teste) | Teto no teste | Como medir |
|---|---|---|---|---|---|
| `--version` | <= 8 MiB | 3,2 -> 3,2 MiB | 5,0 MiB | 8 MiB | `cargo test --test orcamento -- --nocapture` |
| `diag --json` | <= 8 MiB | 3,4 -> 3,4 MiB | 5,6 MiB | 8 MiB | idem |
| `limits` | <= 8 MiB | 3,3 -> 3,3 MiB | 6,0 MiB | 8 MiB | idem |
| `agentes` | <= 8 MiB | 3,5 -> 3,5 MiB | 5,9 MiB | 8 MiB | idem |
| `services` | <= 10,5 MiB | 7,8 -> 7,8 MiB | 8,0 MiB | 10,5 MiB | idem |
| `disco` (varredura de ~disco inteiro) | <= 64 MiB | 28,6 -> 28,6 MiB | não medido no teste (lento, depende da máquina) | n/a | `rss $(which schematize-optimizer) disco` (`wait4` + `ru_maxrss`) |

Teto = medido (debug, o pior caso) + ~30%. O medidor usa `ru_maxrss` do `wait4`, igual ao `VmHWM`; o polling de `/proc/<pid>/status` perde o pico de um processo que vive milissegundos. O número herda o RSS do processo-teste no `exec`, então nunca subestima.

## Diagnóstico

Sem defeito de memória: leitura é por arquivo pequeno de `/proc`, a varredura de disco desce diretório a diretório (só `ReadDir` aberto, sem acumular árvore), não há canal nem cache sem limite. 41 pacotes, 2 duplicatas (`miniz_oxide`), binário 1,4 MiB. `clap`/`serde_json`/`png` custam tempo de build, não RSS (medido: CLI em 3-4 MiB), então ficam. O único ponto acima de 5 MiB é `services` (saída inteira do `systemctl` capturada em `util::run`), ainda sob o teto: registrado como candidato a streaming se crescer.
