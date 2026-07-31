#!/usr/bin/env bash
# Naygo — wrapper de diagnóstico para comandos pesados del agente.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT
#
# Qué hace:
#   1. Registra en logs/agent-bitacora.md el INICIO/FIN del comando con la RAM
#      libre del sistema en cada punto (para correlacionar caídas de la
#      extensión con presión de memoria).
#   2. Redirige TODA la salida del comando a target/agent-out/<slug>.log, así
#      el resultado sobrevive aunque la extensión muera a mitad de la entrega.
#   3. Muestrea la RAM libre cada 5 s en segundo plano → mem.csv junto al log.
#
# Uso:  bash scripts/run-logged.sh <slug> <comando...>
# Ej:   bash scripts/run-logged.sh test-ui cargo test -p naygo-ui-slint

set -u
SLUG="${1:?falta slug}"
shift
OUT_DIR="target/agent-out"
mkdir -p "$OUT_DIR"
OUT="$OUT_DIR/$SLUG.log"
MEM="$OUT_DIR/$SLUG-mem.csv"
BIT="logs/agent-bitacora.md"

free_mb() {
  powershell -NoProfile -Command "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory/1KB" 2>/dev/null | tr -d '\r' | cut -d. -f1
}

# Muestreador de memoria en segundo plano (muere con el wrapper).
( while :; do echo "$(date '+%T'),$(free_mb)" >> "$MEM"; sleep 5; done ) &
SAMPLER=$!
trap 'kill $SAMPLER 2>/dev/null' EXIT

echo "[$(date '+%F %T')] INICIO [$SLUG]: $* (RAM libre: $(free_mb) MB)" >> "$BIT"
"$@" > "$OUT" 2>&1
CODE=$?
echo "[$(date '+%F %T')] FIN [$SLUG]: exit=$CODE (RAM libre: $(free_mb) MB)" >> "$BIT"

# Al terminal solo va el rabo del log (el completo queda en $OUT).
tail -n 15 "$OUT"
echo "---"
echo "log completo: $OUT | memoria: $MEM | exit=$CODE"
exit $CODE
