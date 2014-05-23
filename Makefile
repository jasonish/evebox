all:

clean:
	-grunt clean
	find . -name \*~ -exec rm -f {} \;
	rm -rf dist
	rm -rf build

clean-all:
	-$(MAKE) clean
	rm -rf node_modules
	rm -rf app/bower_components

setup:
	npm install
	bower install
