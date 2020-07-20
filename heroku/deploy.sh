#!/bin/sh

if [ "z$APP" = "z" ];
then
	echo "Set the env var 'APP' to the name of the heroku application"
	exit 1
fi

cargo build --release
cp ../target/release/expression2_image_server .
cp -r ../images images/
heroku container:login
heroku container:push web -a $APP
heroku container:release web -a $APP
