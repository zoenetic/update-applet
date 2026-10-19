# COPR "make srpm" entry point: builds the source RPM from this checkout.
# COPR runs: make -f .copr/Makefile srpm outdir="..." spec="..."
# Network access is available here, so the Rust dependencies are vendored now
# and the RPM build itself runs offline.

spec ?= packaging/fedora/cosmic-applet-updates.spec
outdir ?= $(CURDIR)/srpm
name := cosmic-applet-updates
version := $(shell sed -n 's/^Version:[[:space:]]*//p' $(spec))
sources := $(CURDIR)/.copr/sources

srpm:
	dnf -y install cargo git rpm-build tar xz
	rm -rf $(sources) && mkdir -p $(sources)
	git archive --format=tar.gz --prefix=update-applet-$(version)/ \
		-o $(sources)/update-applet-$(version).tar.gz HEAD
	./packaging/vendor-tarball.sh $(sources)/$(name)-$(version)-vendor.tar.xz
	rpmbuild -bs $(spec) --define "_sourcedir $(sources)" --define "_srcrpmdir $(outdir)"
