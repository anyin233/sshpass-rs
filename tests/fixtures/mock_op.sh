#!/bin/bash
# mock_op.sh — Mimics the 1Password `op` CLI for integration testing.
# Returns canned responses based on arguments.

# Strip --vault <value>, --format <value>, and --tags <value> flags from args for easier matching
ARGS=()
SKIP_NEXT=0
for arg in "$@"; do
    if [[ $SKIP_NEXT -eq 1 ]]; then
        SKIP_NEXT=0
        continue
    fi
    if [[ "$arg" == "--vault" || "$arg" == "--format" || "$arg" == "--tags" ]]; then
        SKIP_NEXT=1
        continue
    fi
    ARGS+=("$arg")
done

CMD="${ARGS[0]}"
SUBCMD="${ARGS[1]}"

case "$CMD" in
    item)
        case "$SUBCMD" in
            list)
                # Check for --tags sshpass-rs
                if [[ "${MOCK_OP_EMPTY:-0}" == "1" ]]; then
                    echo '[]'
                    exit 0
                fi
                echo '[{"id":"abc123","title":"user@host","category":"PASSWORD"},{"id":"def456","title":"root@server","category":"PASSWORD"}]'
                exit 0
                ;;
            get)
                ITEM_ID="${ARGS[2]}"
                case "$ITEM_ID" in
                    abc123)
                        echo '{"id":"abc123","title":"user@host","category":"PASSWORD","fields":[{"id":"password","type":"CONCEALED","value":"s3cret","label":"password"},{"id":"notesPlain","type":"STRING","value":"","label":"notes"}]}'
                        exit 0
                        ;;
                    def456)
                        echo '{"id":"def456","title":"root@server","category":"PASSWORD","fields":[{"id":"password","type":"CONCEALED","value":"r00tpass","label":"password"}]}'
                        exit 0
                        ;;
                    *)
                        echo "[ERROR] 2024/01/01 00:00:00 item not found" >&2
                        exit 1
                        ;;
                esac
                ;;
            create)
                # Read stdin (JSON template), return created item
                cat /dev/stdin > /dev/null 2>&1
                echo '{"id":"new789","title":"created-item","category":"PASSWORD"}'
                exit 0
                ;;
            delete)
                ITEM_ID="${ARGS[2]}"
                case "$ITEM_ID" in
                    abc123|def456|new789)
                        exit 0
                        ;;
                    *)
                        echo "[ERROR] 2024/01/01 00:00:00 item not found" >&2
                        exit 1
                        ;;
                esac
                ;;
            *)
                echo "[ERROR] 2024/01/01 00:00:00 unknown command: $SUBCMD" >&2
                exit 1
                ;;
        esac
        ;;
    *)
        echo "[ERROR] 2024/01/01 00:00:00 unknown command: $CMD" >&2
        exit 1
        ;;
esac
