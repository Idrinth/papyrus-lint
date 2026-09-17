FROM alpine:3.22

RUN apk add --no-cache gcompat libgcc unzip \
    && addgroup -S papyrus-lint \
    && adduser -S -G papyrus-lint papyrus-lint \
    && mkdir -p /project /cache /base-scripts \
    && chown papyrus-lint:papyrus-lint /project /cache /base-scripts

COPY --chmod=755 PapyrusLinterCLI /usr/local/bin/PapyrusLinterCLI
COPY --chmod=755 entrypoint.sh /usr/local/bin/papyrus-lint-entrypoint
RUN PapyrusLinterCLI --version

ENV PAPYRUS_LINT_AST_CACHE_DIR=/cache
VOLUME ["/project", "/cache", "/base-scripts"]
USER papyrus-lint
WORKDIR /project
ENTRYPOINT ["papyrus-lint-entrypoint"]
