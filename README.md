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
