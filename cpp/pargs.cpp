#include <print>

int main(int argc, char**argv){
	for(int i = 0; i < argc; ++i) {
		std::print("arg {}: \"{}\", bytes: ", i, argv[i]);
		for(char*c = argv[i]; *c != '\0'; ++c){
			std::print("{:02x} ", static_cast<unsigned char>(*c));
		}
		std::println("");
	}
}
