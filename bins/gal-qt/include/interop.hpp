#pragma once

#include <cstdint>
#include <functional>
#include <gal.h>

namespace gal::interop
{
    using gal::native::Handle;
    using gal::native::OpenGameStatus;

    struct Context;

    using StartCallback = int (*)(int, char**, Context&);
    int start(StartCallback main, int argc, char** argv);

    struct Context
    {
        Handle m_handle;

        void open_game(const char* config);
    };
} // namespace gal::interop
