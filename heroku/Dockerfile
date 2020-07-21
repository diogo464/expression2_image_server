FROM fedora:latest

EXPOSE 8080

RUN useradd -m server
USER server
WORKDIR /home/server
COPY expression2_image_server e2server
COPY entry.sh entry.sh

ENTRYPOINT ["./entry.sh"]