all:

# Commit the actual bower components used into version control so the
# project can be used as-is after being checked out from version
# control.
vendor-bower-components:
	sed -n 's/.*"\(bower_components.*\)\".*/\1/p' app/index.html | \
		(cd app && xargs git add -f)

	git add -f app/bower_components/bootstrap/dist/fonts

package:
	grunt package

# Basic clean - build artifacts, backup files...
clean:
	-grunt clean
	find . -name \*~ -exec rm -f {} \;
	rm -rf dist
	rm -rf build

# Basic clean plus anything pulled down by build tools.
clean-all:
	-$(MAKE) clean
	rm -rf node_modules
	rm -rf app/bower_components

dev-setup:
	npm install
	bower install
