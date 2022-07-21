#include <interop.hpp>
#include <thread>

using namespace gal::interop;

int main_impl(int argc, char** argv, Context& context)
{
    context.open_game(argv[1]);
    using namespace std::chrono;
    std::this_thread::sleep_for(20s);
    return 0;
}

int main(int argc, char** argv)
{
    return start(main_impl, argc, argv);
}
