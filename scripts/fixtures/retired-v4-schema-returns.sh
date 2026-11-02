# expect-step: no retired v4 in the detector
# expect-text: the retired v4 contract is back in contract/
# ADR 2026-11-02-retirar-v4: the retired schema is not a file of the contract
# again. The guardian sees the file name, so the content is only a placeholder.
mkdir -p contract/schema
printf '{}\n' > contract/schema/calipso.scene.v4.json
