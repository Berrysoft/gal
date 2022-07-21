#include <QDebug>
#include <interop.hpp>

namespace gal::interop
{
    struct init_data
    {
        StartCallback main;
        int argc;
        char** argv;
    };

    static int fn_init_callback(Handle handle, void* data)
    {
        auto d = reinterpret_cast<init_data*>(data);
        Context context{ handle };
        return (d->main)(d->argc, d->argv, context);
    }

    int start(StartCallback main, int argc, char** argv)
    {
        init_data data{ main, argc, argv };
        return gal::native::start("com.berrysoft.gal", fn_init_callback, &data);
    }

    static void fn_open_game_callback(OpenGameStatus status, const void* status_data, void* data)
    {
        switch (status)
        {
        case OpenGameStatus::LoadSettings:
            qInfo("LoadSettings");
            break;
        case OpenGameStatus::LoadProfile:
            qInfo("LoadProfile");
            break;
        case OpenGameStatus::CreateRuntime:
            qInfo("CreateRuntime");
            break;
        case OpenGameStatus::LoadPlugin:
        {
            auto load_plugin = reinterpret_cast<const gal::native::OpenGameLoadPlugin*>(status_data);
            qInfo() << "Loading plugin" << QString::fromUtf8(load_plugin->name, load_plugin->name_len) << QStringLiteral("(%1/%2)").arg(load_plugin->index).arg(load_plugin->len);
            break;
        }
        case OpenGameStatus::LoadRecords:
            qInfo("LoadRecords");
            break;
        case OpenGameStatus::Loaded:
            qInfo("Loaded");
            break;
        }
    }

    void Context::open_game(const char* config)
    {
        gal::native::open_game(m_handle, config, fn_open_game_callback, nullptr);
    }
} // namespace gal::interop
