// Example: using OP2Formats from C++ through its C interface.
//
//   op2formats_example [game folder] [map]
//
// Opens the game (the folder given, else $OP2_DIR or an install it finds), prints a map's size and tilesets, the
// colour of its top-left tile and the Scout's row from vehicles.txt. Build: see op2formats-c/README.md.

#include <op2formats.h>

#include <cstdio>
#include <memory>
#include <string>
#include <vector>

namespace {

// Handles in unique_ptrs so they are freed on every path.
struct GameFree { void operator()(Op2fGame* p) const { op2f_game_free(p); } };
struct MapFree { void operator()(Op2fMap* p) const { op2f_map_free(p); } };
struct TilesetFree { void operator()(Op2fTileset* p) const { op2f_tileset_free(p); } };
struct SheetFree { void operator()(Op2fSheet* p) const { op2f_sheet_free(p); } };
using Game = std::unique_ptr<Op2fGame, GameFree>;
using Map = std::unique_ptr<Op2fMap, MapFree>;
using Tileset = std::unique_ptr<Op2fTileset, TilesetFree>;
using Sheet = std::unique_ptr<Op2fSheet, SheetFree>;

std::string lastError() {
    std::string s(op2f_last_error(nullptr, 0), '\0');
    op2f_last_error(s.data(), s.size() + 1);
    return s;
}

bool check(Op2fStatus status, const char* what) {
    if (status != OP2F_STATUS_OK)
        std::fprintf(stderr, "%s failed (status %d): %s\n", what, static_cast<int>(status), lastError().c_str());
    return status == OP2F_STATUS_OK;
}

// Calls an op2f string getter twice: once for the length, once to copy.
template <typename F> std::string getString(F f) {
    size_t len = 0;
    if (f(nullptr, 0, &len) != OP2F_STATUS_OK) return {};
    std::string s(len, '\0');
    f(s.data(), len + 1, &len);
    return s;
}

}  // namespace

int main(int argc, char** argv) {
    std::printf("OP2Formats %s\n", op2f_version());

    Op2fGame* rawGame = nullptr;
    if (!check(op2f_game_open(argc > 1 ? argv[1] : nullptr, &rawGame), "opening the game")) return 1;
    Game game(rawGame);
    std::printf("game folder: %s\n", getString([&](char* b, size_t c, size_t* l) { return op2f_game_dir(game.get(), b, c, l); }).c_str());
    std::printf("maps: %zu\n", op2f_game_map_count(game.get()));

    const char* mapName = argc > 2 ? argv[2] : "cm01.map";
    Op2fMap* rawMap = nullptr;
    if (!check(op2f_map_open(game.get(), mapName, &rawMap), "opening the map")) return 1;
    Map map(rawMap);
    std::printf("%s: %ux%u tiles%s, %zu tile mappings\n", mapName, op2f_map_width(map.get()), op2f_map_height(map.get()),
                op2f_map_wraps(map.get()) ? " (wraps)" : "", op2f_map_mapping_count(map.get()));
    for (size_t i = 0; i < op2f_map_tileset_count(map.get()); ++i) {
        uint32_t tiles = 0;
        std::string name = getString([&](char* b, size_t c, size_t* l) { return op2f_map_tileset(map.get(), i, b, c, l, &tiles); });
        if (!name.empty()) std::printf("  tileset %zu: %s (%u tiles)\n", i, name.c_str(), tiles);
    }

    // The top-left tile: its mapping names a tileset slot and a tile in it; average that tile's colour.
    Op2fTile tile;
    Op2fTileMapping mapping;
    if (!check(op2f_map_tile(map.get(), 0, 0, &tile), "reading tile (0, 0)") ||
        !check(op2f_map_mapping(map.get(), tile.mapping_index, &mapping), "reading its mapping"))
        return 1;
    std::string setName = getString([&](char* b, size_t c, size_t* l) { return op2f_map_tileset(map.get(), mapping.tileset, b, c, l, nullptr); });
    Op2fTileset* rawTileset = nullptr;
    if (!check(op2f_tileset_open(game.get(), setName.c_str(), &rawTileset), "opening the tileset")) return 1;
    Tileset tileset(rawTileset);
    std::vector<uint8_t> rgba(OP2F_TILE_RGBA_BYTES);
    if (!check(op2f_tileset_tile_rgba(tileset.get(), mapping.graphic, rgba.data(), rgba.size()), "decoding the tile")) return 1;
    size_t sum[3] = {};
    for (size_t p = 0; p < rgba.size(); p += 4)
        for (int c = 0; c < 3; ++c) sum[c] += rgba[p + c];
    const size_t pixels = rgba.size() / 4;
    std::printf("tile (0, 0): cell type %u, %s tile %u, average colour #%02zx%02zx%02zx\n", tile.cell_type, setName.c_str(),
                mapping.graphic, sum[0] / pixels, sum[1] / pixels, sum[2] / pixels);

    Op2fSheet* rawSheet = nullptr;
    if (!check(op2f_sheet_open(game.get(), "vehicles.txt", &rawSheet), "reading vehicles.txt")) return 1;
    Sheet sheet(rawSheet);
    size_t row = 0;
    if (!check(op2f_sheet_find_row(sheet.get(), "Scout", &row), "finding the Scout")) return 1;
    std::printf("vehicles.txt, Scout:\n");
    const size_t columns = op2f_sheet_cell_count(sheet.get(), row);
    for (size_t col = 0; col < columns && col < 8; ++col) {
        std::string header = getString([&](char* b, size_t c, size_t* l) { return op2f_sheet_column_name(sheet.get(), col, b, c, l); });
        std::string cell = getString([&](char* b, size_t c, size_t* l) { return op2f_sheet_cell(sheet.get(), row, col, b, c, l); });
        std::printf("  %-20s %s\n", header.c_str(), cell.c_str());
    }
    return 0;
}
