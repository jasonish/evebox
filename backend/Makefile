GOHOSTARCH :=	$(shell go env GOHOSTARCH)
GOHOSTOS :=	$(shell go env GOHOSTOS)

BUILD_DATE :=	$(shell TZ=UTC date)
LDFLAGS :=	-X \"main.buildDate=$(BUILD_DATE)\"

APP :=	evebox

export GO15VENDOREXPERIMENT=1
export CGO_ENABLED=0

all:
	go build -ldflags "$(LDFLAGS)" -o ${APP}
	rice -v append --exec ${APP}

install-deps:
	go get github.com/Masterminds/glide
	go get github.com/GeertJohan/go.rice/rice
	glide install

dist: GOARCH ?= $(shell go env GOARCH)
dist: GOOS ?= $(shell go env GOOS)
dist:
	go build -ldflags "$(LDFLAGS)" -o ${APP}-${GOOS}-${GOARCH}/${APP}
	rice -v append --exec ${APP}-${GOOS}-${GOARCH}/${APP}
	zip -r ${APP}-${GOOS}-${GOARCH}.zip ${APP}-${GOOS}-${GOARCH}

release:
	GOOS=linux GOARCH=amd64 $(MAKE) dist
	GOOS=freebsd GOARCH=amd64 $(MAKE) dist
	GOOS=darwin GOARCH=amd64 $(MAKE) dist
	GOOS=windows GOARCH=amd64 $(MAKE) dist

VERSION := $(shell date +%Y%m%d%H%M)

deb:
	fpm -s dir \
		-C evebox-linux-amd64 \
		-t deb \
		-n evebox \
		-v $(VERSION) \
		--prefix /usr/bin \
		evebox

rpm:
	fpm -s dir \
		-C evebox-linux-amd64 \
		-t rpm \
		-n evebox \
		-v $(VERSION) \
		--prefix /usr/bin \
		evebox

clean:
	rm -f ${APP}

distclean: clean
	rm -rf vendor
