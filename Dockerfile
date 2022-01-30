FROM rust:1.58.1-alpine3.15

ENV APP=queueleuleu
ENV APP_USER=queueleuleu

WORKDIR /${APP}
COPY ./target/debug/${APP} ./${APP}

EXPOSE 80

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER

CMD ["./${APP}"]
