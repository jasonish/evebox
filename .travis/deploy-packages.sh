#! /bin/bash

set -e

REPO_ROOT="https://api.bintray.com/content/jasonish"

if [ "${BINTRAY_API_KEY}" = "" ]; then
    echo "BINTRAY_API_KEY is empty. Not deploying."
    exit 0
fi

deploy_development() {
    # Deploy zip's to Bintray.
    for zip in dist/evebox-*.zip; do
	version=`echo $(basename ${zip}) | sed -n 's/evebox-\([^-]\+\).*/\1/p'`
	echo "Uploading ${zip}."
	curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development/evebox/${version}/$(basename ${zip})"	 
	echo

	# A bit crude, but also upload with the version of "latest".
	dest_filename=$(echo $(basename ${zip}) | sed -e "s#${version}#latest#g")
	echo "Uploading ${zip} to ${dest_filename}."
	curl -T ${zip} -u jasonish:${BINTRAY_API_KEY} \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development/evebox/latest/${dest_filename}"
	echo

    done
}

deploy_development_rpm() {
    # Deploy RPM to Bintray.
    for rpm in $(ls dist/evebox*.rpm); do
	echo "Uploading ${rpm}."
	version=`rpm -qp --queryformat '%{VERSION}%{RELEASE}' "${rpm}"`
	curl -T ${rpm} -u jasonish:${BINTRAY_API_KEY} \
	     "${REPO_ROOT}/evebox-development-rpm-x86_64/evebox/${version}/$(basename ${rpm})?publish=1&override=1"
	
	echo
	break
    done
}

deploy_development_debian() {
    for deb in $(ls dist/evebox*_amd64.deb); do
	echo "Uploading ${deb}."
	version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`

	# The old repo - SELKS users may still be downloading from this
	# one.
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/deb-evebox-latest/evebox/${version}/$(basename ${deb})"
	echo

	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: amd64" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
	echo
	
    done

    for deb in $(ls dist/evebox*_i386.deb); do
	echo "Uploading ${deb}."
	version=`dpkg -I "${deb}" | awk '/Version:/ { print $2 }'`
	curl -T "${deb}" -u "jasonish:${BINTRAY_API_KEY}" \
	     -H "X-Bintray-Debian-Distribution: jessie" \
	     -H "X-Bintray-Debian-Component: main" \
	     -H "X-Bintray-Debian-Architecture: i386" \
	     -H "X-Bintray-Override: 1" \
	     -H "X-Bintray-Publish: 1" \
	     "${REPO_ROOT}/evebox-development-debian/evebox/${version}/$(basename ${deb})"
    done
    
}

# Only deploy if building from master on my repo.
if [ "${TRAVIS_REPO_SLUG}" != "jasonish/evebox" ]; then
    echo "Not deploying packages for builds from repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

if [ "${TRAVIS_BRANCH}" = "master" ]; then
    deploy_development
    deploy_development_rpm
    deploy_development_debian
fi
