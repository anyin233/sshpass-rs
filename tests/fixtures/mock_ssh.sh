#!/bin/bash
# mock_ssh.sh — Mimics ssh for integration testing of alias resolution.

# Check if -G flag is present (config dump mode)
for arg in "$@"; do
    if [[ "$arg" == "-G" ]]; then
        # Find the destination (last non-flag argument)
        DEST=""
        SKIP_NEXT=0
        for a in "$@"; do
            if [[ $SKIP_NEXT -eq 1 ]]; then
                SKIP_NEXT=0
                continue
            fi
            if [[ "$a" == "-G" ]]; then
                continue
            fi
            if [[ "$a" == "-F" ]]; then
                SKIP_NEXT=1
                continue
            fi
            if [[ ! "$a" =~ ^- ]]; then
                DEST="$a"
            fi
        done

        case "$DEST" in
            myalias)
                echo "user testuser"
                echo "hostname 10.0.0.1"
                echo "port 22"
                echo "addressfamily any"
                exit 0
                ;;
            gw)
                echo "user admin"
                echo "hostname gateway.local"
                echo "port 22"
                echo "addressfamily any"
                exit 0
                ;;
            custom-alias)
                echo "user customuser"
                echo "hostname custom.example.com"
                echo "port 22"
                exit 0
                ;;
            *)
                exit 255
                ;;
        esac
    fi
done

# Non -G mode: just exit 0 (we don't need real SSH behavior for these tests)
exit 0
