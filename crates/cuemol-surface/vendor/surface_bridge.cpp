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
};

static CueMolSurface meshms_surface(const double *xyzr, unsigned count, double density, double probe) {
    std::vector<std::array<double,4>> atoms(count);
    for (unsigned i=0; i<count; ++i) std::copy(xyzr+i*4, xyzr+i*4+4, atoms[i].begin());
    auto result = meshms::remove_flaps(meshms::build_mesh_from_cache(
        meshms::compute_rs_from_array(atoms, probe), 1.18/std::sqrt(density), true));
    if (result.verts.empty() || result.faces.empty()) throw std::runtime_error("MeshMS returned an empty surface");
    CueMolSurface output;
    output.backend = 1;
    output.vertices.reserve(result.verts.size()*6);
    output.triangles.reserve(result.faces.size()*3);
    for (std::size_t i=0; i<result.verts.size(); ++i) {
        output.vertices.insert(output.vertices.end(), result.verts[i].begin(), result.verts[i].end());
        output.vertices.insert(output.vertices.end(), result.vnormals[i].begin(), result.vnormals[i].end());
    }
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

extern "C" {
CueMolSurface *cuemol_surface_build(const double *xyzr, unsigned count, double density,
                                   double probe, unsigned backend, char *error, unsigned error_size) noexcept {
    try {
        CueMolSurface output;
        if (backend == 2) output = ball_surface(xyzr,count,density,probe);
        else {
            try { output = meshms_surface(xyzr,count,density,probe); }
            catch (...) { output = ball_surface(xyzr,count,density,probe); }
        }
        for (double value : output.vertices)
            if (!std::isfinite(value)) throw std::runtime_error("Surface contains a non-finite vertex");
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
const unsigned *cuemol_surface_triangles(const CueMolSurface *surface) {return surface->triangles.data();}
}
