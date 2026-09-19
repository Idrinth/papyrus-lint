FROM alpine:3.24

# One published image per built-in preset (strict/standard/careful).
ARG PRESET=strict
# Overwrite base files with extender files
ARG EXTENDER=no

RUN apk add --no-cache gcompat libgcc unzip \
    && addgroup -S papyrus-lint \
    && adduser -S -G papyrus-lint papyrus-lint \
    && mkdir -p /project /cache /base-scripts/usr/local/share/papyrus-lint \
    && chown papyrus-lint:papyrus-lint /project /cache /base-scripts

COPY --chmod=755 PapyrusLinterCLI /usr/local/bin/PapyrusLinterCLI
COPY --chmod=755 entrypoint.sh /usr/local/bin/papyrus-lint-entrypoint
RUN PapyrusLinterCLI --version

# Generate the selected preset's config and point lookup roots at the
# container paths documented in the README (/project, /cache, /base-scripts).
# The file next to the binary is the executable-adjacent base config `init`
# layers on; the copy under /usr/local/share is what the entrypoint passes
# to --config when /project has no papyrus-lint.yaml/.yml of its own.
WORKDIR /tmp/papyrus-lint-init
RUN case "$PRESET" in \
        strict|standard|careful) ;; \
        *) echo "unsupported PRESET '$PRESET' (expected strict, standard, or careful)" >&2; exit 1 ;; \
    esac \
    && PapyrusLinterCLI init --preset "$PRESET" \
    && awk ' \
        BEGIN { replaced = 0 } \
        /^lookup_script_roots:/ { \
            print "lookup_script_roots: [\"/base-scripts\"]"; \
            replaced = 1; \
            in_list = 1; \
            next; \
        } \
        in_list && /^[[:space:]]*-/ { next } \
        in_list && /^[^[:space:]]/ { in_list = 0 } \
        { print } \
        END { if (!replaced) print "lookup_script_roots: [\"/base-scripts\"]" } \
    ' papyrus-lint.yaml > papyrus-lint.yaml.adjusted \
    && cp papyrus-lint.yaml.adjusted /usr/local/share/papyrus-lint/papyrus-lint.yaml \
    && cp papyrus-lint.yaml.adjusted /usr/local/bin/papyrus-lint.yaml \
    && rm -rf /tmp/papyrus-lint-init

ENV PAPYRUS_LINT_AST_CACHE_DIR=/cache
ENV PAPYRUS_LINT_PRESET=${PRESET}
VOLUME ["/project", "/cache", "/base-scripts"]
USER papyrus-lint
WORKDIR /project
ENTRYPOINT ["papyrus-lint-entrypoint"]
