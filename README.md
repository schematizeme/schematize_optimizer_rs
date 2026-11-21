# schematize optimizer

Deixa a máquina de dev **previsível**: mede o ambiente e põe build, navegador e container
cada um no seu teto de recurso.

```sh
optimizer diag              # mede e sugere. NÃO muda nada.
optimizer caixas            # mostra os tetos que aplicaria
optimizer caixas --aplicar  # aplica
optimizer caixas --revert   # desfaz tudo o que ele fez
```

---

## O piso que este app tem e os outros não precisaram

O `schematize disco` apaga **lixo recriável**: no pior caso, um build recompila. O
`deployer` mexe em **credencial**: no pior caso, alguém rotaciona um token. Este mexe na
**máquina** — e no pior caso ela não liga.

Daí quatro regras que valem em cada linha ([ADR-0011]):

1. **Sugerir é o padrão; aplicar é explícito.**
2. **Nada que não se saiba desfazer.** É por isso que as caixas se **recusam** a sobrescrever
   um `.slice` que não geraram — o `--revert` não saberia o que havia antes.
3. **Detectar antes de propor.** Nada de sugerir `amdgpu.gttsize` num Intel, onde o knob não
   existe: a ferramenta diz o que **não** dá, em vez de fingir.
4. **Nada de heurística sobre o que a pessoa usa.** Desabilitar bluetooth "porque parece
   inútil" numa máquina de teclado bluetooth deixa alguém sem teclado.

## A memória compartilhada de vídeo

O `amdgpu` deixa a GPU mapear **metade da sua RAM** por padrão. Numa máquina de 16 GB, são
8 GB que podem sumir num pico enquanto um build precisa deles — e é essa a falta de
previsibilidade que causa travamento.

`optimizer diag` mede e mostra o número. **Aplicar ainda não é possível por aqui:** mexe no
cmdline do kernel, e é a única mudança deste app que pode deixar a máquina sem bootar. Ela só
entra com as quatro mitigações do ADR-0011 §5 — entrada de GRUB **adicional** (a original
intacta), backup, um parâmetro por vez, e recusa em bootloader que não seja GRUB.

| GPU | dá para limitar? |
|---|---|
| **AMD** | sim — `amdgpu.gttsize`, no cmdline do kernel |
| **Intel** | **não** — é fixado pelo firmware (DVMT), no setup da UEFI |
| **NVIDIA discreta** | não se aplica — usa VRAM própria, não GTT |

## As caixas

`MemoryHigh` é onde o kernel começa a **apertar** (fica lento); `MemoryMax` é onde ele
**mata**. A distância entre os dois é o que troca *"o build morreu depois de uma hora"* por
*"o build demorou mais"* — e é por isso que eles nunca saem colados.

O build **não** ganha teto de CPU: build lento por cota é o oposto do que se quer. O que se
quer é que ele não coma a RAM toda.

Os tetos valem para o que for **iniciado dentro** do slice:

```sh
systemd-run --user --slice=dev-build.slice --scope cargo build
```

## Jaulas: um teto por execução

As caixas acima são slices **globais**: um teto compartilhado por tudo o que roda dentro do
slice. Se um comando estoura, o OOM escolhe uma vítima entre todos e leva a operação inteira.
A **jaula** é o oposto, **descentralizada**: cada comando roda no próprio cgroup, com teto duro
de 500 MiB a 2 GiB, e estourar mata **só ele**, com veredito claro.

```sh
optimizer jaula estimar [--tipo agente|script|tool|teste|build|navegador]   # puro, não executa
optimizer jaula rodar [--classe leve|medio|pesado | --mem-mib N] [--timeout-s N] \
                      [--nome X] [--json] -- <comando...>
optimizer jaula diag                                  # backend + o que falta para ativar
```

| saída de `jaula rodar` | significa |
|---|---|
| `0` | concluiu |
| código do filho | o comando falhou sozinho |
| `137` | **reservado**: passou do teto de memória e foi encerrado; o resto da máquina não foi tocado |
| `124` | passou do `--timeout-s` |
| `2` | sem como impor teto aqui: **nada foi executado** (veja `jaula diag`) |

`--mem-mib` acima de 2048 só com `--adr <id>`; sem ele o pedido é recusado com a explicação.
Com `--json` o stdout é só o veredito (`status`, `exit_code`, `ceiling_mib`, `peak_mib`) e o
stdout do comando vai para o stderr.

**Por que `MemoryHigh` sozinho trava sem swap.** `MemoryHigh` só faz *throttle*: sem swap, a
memória anônima não tem para onde ir, o processo fica engasgado acima do limiar e o OOM nunca
chega, então a execução (e a máquina junto) trava em vez de morrer. A jaula usa `MemoryMax`
(corte duro) + `MemorySwapMax=0`, e a lib `schematize_plataforma` (feature `jaula`) mata a
unidade que encalha no high. Sem systemd scope com o controller `memory` delegado, o fallback é
`prlimit` (mais fraco, sem prova de OOM); sem nenhum dos dois, nada roda sem teto.

## Estado

| fase | | |
|---|---|---|
| 0 | ADR-0011 | **feito** |
| 1 | Repo + CI desde o commit 1 | **feito** |
| 2 | Diagnóstico (só leitura) | **feito** |
| 3 | Caixas + revert | **feito** |
| 4 | Serviços inertes, com allowlist justificada item a item | aberto |
| 5 | GTT/VRAM (bootloader), com as 4 mitigações | aberto |
| 6 | `install.sh --optimizer`, `schematize optimizer`, updater | aberto |

[ADR-0011]: ../schematize_app_archive/decisoes/ADR-0011-optimizer-app.md
