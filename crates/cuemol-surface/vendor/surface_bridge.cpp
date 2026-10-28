#include "edtsurf/CommonPara.h"
#include "edtsurf/ProteinSurface.h"
#include <algorithm>
#include <cstdint>
#include <meshms/meshms.hpp>
#include <BALL/common.h>
#include <BALL/STRUCTURE/reducedSurface.h>
#include <BALL/STRUCTURE/solventExcludedSurface.h>
#include <BALL/STRUCTURE/triangulatedSES.h>
#include <array>
#include <cmath>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <vector>

struct CueMolSurface {
    std::vector<double> vertices;
    std::vector<unsigned> triangles;
    unsigned backend;
    std::vector<unsigned> owners;
};

static CueMolSurface meshms_surface(const double *xyzr, unsigned count, double mesh_size, double probe) {
    std::vector<std::array<double,4>> atoms(count);
    for (unsigned i=0; i<count; ++i) std::copy(xyzr+i*4, xyzr+i*4+4, atoms[i].begin());
    auto result = meshms::remove_flaps(meshms::build_mesh_from_cache(
        meshms::compute_rs_from_array(atoms, probe), mesh_size, true));
    if (result.verts.empty() || result.faces.empty()) throw std::runtime_error("MeshMS returned an empty surface");
    CueMolSurface output;
    output.backend = 1;
    if (result.verts.size() > 48000000 || result.faces.size() > 96000000)
        throw std::runtime_error("MeshMS surface exceeds the vertex budget");
    output.vertices.reserve(result.verts.size()*6);
    output.triangles.reserve(result.faces.size()*3);
    for (std::size_t i=0; i<result.verts.size(); ++i) {
        output.vertices.insert(output.vertices.end(), result.verts[i].begin(), result.verts[i].end());
        output.vertices.insert(output.vertices.end(), result.vnormals[i].begin(), result.vnormals[i].end());
    }
    output.owners.reserve(result.atom_id.size());
    for (const auto owner : result.atom_id)
        output.owners.push_back(owner > 0 && owner <= count ? owner - 1 : UINT32_MAX);
    for (const auto &face : result.faces) output.triangles.insert(output.triangles.end(), face.begin(), face.end());
    return output;
}

static CueMolSurface ball_surface(const double *xyzr, unsigned count, double density, double probe) {
    std::vector<BALL::TSphere3<double>> spheres;
    spheres.reserve(count);
    for (unsigned i=0; i<count; ++i)
        spheres.emplace_back(BALL::TVector3<double>(xyzr[i*4],xyzr[i*4+1],xyzr[i*4+2]),xyzr[i*4+3]);
    const double step = probe < 1.5 ? 0.01 : -0.01;
    for (unsigned retry=0; retry<10; ++retry) {
        BALL::ReducedSurface reduced(spheres, probe + retry*step);
        reduced.compute();
        BALL::SolventExcludedSurface excluded(&reduced);
        excluded.compute();
        if (!excluded.check()) continue;
        BALL::TriangulatedSES surface(&excluded, density);
        surface.compute();
        CueMolSurface output;
        output.backend = 2;
        output.vertices.reserve(surface.getNumberOfPoints()*6);
        output.triangles.reserve(surface.getNumberOfTriangles()*3);
        unsigned index=0;
        for (auto it=surface.beginPoint(); it!=surface.endPoint(); ++it) {
            auto &point=**it;
            output.vertices.insert(output.vertices.end(), {point.point_.x,point.point_.y,point.point_.z,
                                                           point.normal_.x,point.normal_.y,point.normal_.z});
            point.setIndex(index++);
        }
        for (auto it=surface.beginTriangle(); it!=surface.endTriangle(); ++it)
            for (unsigned i=0; i<3; ++i) output.triangles.push_back((*it)->getVertex(i)->getIndex());
        if (output.vertices.empty() || output.triangles.empty()) throw std::runtime_error("BALL returned an empty surface");
        return output;
    }
    throw std::runtime_error("BALL could not construct a valid solvent-excluded surface");
}

static CueMolSurface edtsurf_surface(const double *xyzr, unsigned count, double fixsf, double probe, unsigned type) {
    if (!count) throw std::runtime_error("EDTSurf needs at least one atom");
    edtsurf::ProteinSurface surface;
    std::vector<double> radii;
    std::vector<edtsurf::atom> atoms(count);
    for (unsigned i=0; i<count; ++i) {
        const double radius = xyzr[i*4+3];
        auto found = std::find(radii.begin(), radii.end(), radius);
        const unsigned index = found - radii.begin();
        if (found == radii.end()) radii.push_back(radius);
        if (radii.size() > edtsurf::ProteinSurface::NO_RAD_TYPES)
            throw std::runtime_error("too many EDTSurf radius types");
        atoms[i].simpletype = 1;
        atoms[i].seqno = i;
        atoms[i].detail = index;
        atoms[i].x = static_cast<float>(xyzr[i*4]);
        atoms[i].y = static_cast<float>(xyzr[i*4+1]);
        atoms[i].z = static_cast<float>(xyzr[i*4+2]);
        atoms[i].ins = ' ';
    }
    for (int i=0; i<edtsurf::ProteinSurface::NO_RAD_TYPES; ++i)
        surface.rasrad[i] = i < radii.size() ? radii[i] : 1.7;
    surface.proberadius = probe;
    surface.fixsf = fixsf;
    surface.initpara(0, count-1, atoms.data(), type == 2, type != 0);
    surface.fillvoxels(0, count-1, type == 2, atoms.data(), true);
    surface.buildboundary();
    if (type == 2) surface.fastdistancemap();
    surface.marchingcube(type == 0 ? 1 : type == 1 ? 3 : 4);
    // EDTSurf leaves this smoothing flag uninitialized after marching cubes.
    for (int i=0; i<surface.vertnumber; ++i) surface.verts[i].inout = false;
    surface.laplaciansmooth(1);
    surface.computenorm();
    CueMolSurface output;
    output.backend = 3 + type;
    output.vertices.reserve(surface.vertnumber * 6);
    output.owners.reserve(surface.vertnumber);
    output.triangles.reserve(surface.facenumber * 3);
    for (int i=0; i<surface.vertnumber; ++i) {
        const auto &v = surface.verts[i];
        output.vertices.insert(output.vertices.end(), {
            static_cast<float>(v.x / surface.scalefactor - surface.ptran.x),
            static_cast<float>(v.y / surface.scalefactor - surface.ptran.y),
            static_cast<float>(v.z / surface.scalefactor - surface.ptran.z),
            static_cast<float>(v.pn.x), static_cast<float>(v.pn.y), static_cast<float>(v.pn.z)});
        output.owners.push_back(v.atomid < 0 ? UINT32_MAX : static_cast<unsigned>(v.atomid));
    }
    for (int i=0; i<surface.facenumber; ++i) {
        const auto &f = surface.faces[i];
        output.triangles.insert(output.triangles.end(), {static_cast<unsigned>(f.a), static_cast<unsigned>(f.b), static_cast<unsigned>(f.c)});
    }
    return output;
}

extern "C" {
CueMolSurface *cuemol_surface_build(const double *xyzr, unsigned count, double density,
                                   double probe, unsigned backend, char *error, unsigned error_size) noexcept {
    try {
        CueMolSurface output;
        if (backend >= 3 && backend <= 5) output = edtsurf_surface(xyzr,count,density,probe,backend-3);
        else if (backend == 6) output = meshms_surface(xyzr,count,density,probe);
        else if (backend == 2) output = ball_surface(xyzr,count,density,probe);
        else {
            try { output = meshms_surface(xyzr,count,1.18/std::sqrt(density),probe); }
            catch (...) { output = ball_surface(xyzr,count,density,probe); }
        }
        for (double value : output.vertices)
            if (!std::isfinite(value)) throw std::runtime_error("Surface contains a non-finite vertex");
        output.owners.resize(output.vertices.size()/6, UINT32_MAX);
        return new CueMolSurface(std::move(output));
    } catch (const std::exception &exception) {
        if (error_size) {std::strncpy(error,exception.what(),error_size-1);error[error_size-1]=0;}
    } catch (const BALL::Exception::GeneralException &exception) {
        if (error_size) {std::strncpy(error,exception.getMessage(),error_size-1);error[error_size-1]=0;}
    } catch (...) {
        if (error_size) {std::strncpy(error,"Surface generation failed",error_size-1);error[error_size-1]=0;}
    }
    return nullptr;
}
void cuemol_surface_destroy(CueMolSurface *surface) { delete surface; }
unsigned cuemol_surface_vertex_count(const CueMolSurface *surface) {return surface->vertices.size()/6;}
unsigned cuemol_surface_triangle_count(const CueMolSurface *surface) {return surface->triangles.size()/3;}
unsigned cuemol_surface_backend(const CueMolSurface *surface) {return surface->backend;}
const double *cuemol_surface_vertices(const CueMolSurface *surface) {return surface->vertices.data();}
const unsigned *cuemol_surface_owners(const CueMolSurface *surface) {return surface->owners.data();}
const unsigned *cuemol_surface_triangles(const CueMolSurface *surface) {return surface->triangles.data();}
}
